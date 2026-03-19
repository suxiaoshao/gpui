use crate::{
    CaptureError,
    capture::{DisplayId, ImageFrame, ScreenRect},
};
use std::sync::mpsc;
use std::time::Duration;
use windows::{
    Foundation::TypedEventHandler,
    Graphics::{
        Capture::{
            Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureAccess,
            GraphicsCaptureAccessKind, GraphicsCaptureItem, GraphicsCaptureSession,
        },
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
        Imaging::{BitmapAlphaMode, SoftwareBitmap},
    },
    Security::Authorization::AppCapabilityAccess::AppCapabilityAccessStatus,
    Win32::{
        Foundation::{E_ACCESSDENIED, HMODULE, LPARAM, RECT},
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL},
            Direct3D11::{D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice},
            Dxgi::{IDXGIAdapter, IDXGIDevice},
            Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW},
        },
        System::WinRT::{
            Direct3D11::CreateDirect3D11DeviceFromDXGIDevice,
            Graphics::Capture::IGraphicsCaptureItemInterop,
        },
    },
    core::{BOOL, IInspectable, Interface, Ref},
};

pub(crate) mod async_support;
pub(crate) mod image_frame;

use async_support::wait_async_operation;
use image_frame::{clip_rect_to_image, crop_image_frame, rgba_frame_from_software_bitmap};

pub(super) fn capture_display(display_id: DisplayId) -> Result<ImageFrame, CaptureError> {
    ensure_capture_support()?;
    let monitor = monitor_for_display_id(display_id)?;
    let item = capture_item_for_monitor(monitor)?;
    let bitmap = capture_software_bitmap(&item)?;
    rgba_frame_from_software_bitmap(&bitmap).map_err(CaptureError::SystemFailure)
}

pub(super) fn capture_rect(display_id: DisplayId, rect: ScreenRect) -> Result<ImageFrame, CaptureError> {
    let display = capture_display(display_id)?;
    let clipped = clip_rect_to_image(display.width, display.height, rect).ok_or(
        CaptureError::InvalidInput("capture rectangle does not intersect the display"),
    )?;
    crop_image_frame(&display, clipped).map_err(CaptureError::SystemFailure)
}

fn ensure_capture_support() -> Result<(), CaptureError> {
    let is_supported = GraphicsCaptureSession::IsSupported().map_err(map_capture_error)?;
    if !is_supported {
        return Err(CaptureError::BackendUnavailable(
            "windows graphics capture is not supported",
        ));
    }

    let status = wait_async_operation(
        GraphicsCaptureAccess::RequestAccessAsync(GraphicsCaptureAccessKind::Programmatic)
            .map_err(map_capture_error)?,
    )
    .map_err(map_capture_error)?;
    match status {
        AppCapabilityAccessStatus::Allowed
        | AppCapabilityAccessStatus::UserPromptRequired
        | AppCapabilityAccessStatus::NotDeclaredByApp => Ok(()),
        AppCapabilityAccessStatus::DeniedByUser | AppCapabilityAccessStatus::DeniedBySystem => {
            Err(CaptureError::PermissionDenied)
        }
        _ => Err(CaptureError::BackendUnavailable(
            "windows graphics capture access is unavailable",
        )),
    }
}

fn capture_software_bitmap(item: &GraphicsCaptureItem) -> Result<SoftwareBitmap, CaptureError> {
    let size = item.Size().map_err(map_capture_error)?;
    let device = create_direct3d_device()?;
    let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        1,
        size,
    )
    .map_err(map_capture_error)?;
    let session = frame_pool
        .CreateCaptureSession(item)
        .map_err(map_capture_error)?;
    let (tx, rx) = mpsc::channel::<windows_core::Result<Direct3D11CaptureFrame>>();
    let handler = TypedEventHandler::new(move |sender: Ref<Direct3D11CaptureFramePool>, _| {
        if let Some(sender) = sender.as_ref() {
            let _ = tx.send(sender.TryGetNextFrame());
        }
        Ok(())
    });
    let token = frame_pool
        .FrameArrived(&handler)
        .map_err(map_capture_error)?;

    session.StartCapture().map_err(map_capture_error)?;
    let frame_result = rx.recv_timeout(Duration::from_secs(5)).map_err(|_| {
        let _ = frame_pool.RemoveFrameArrived(token);
        let _ = session.Close();
        let _ = frame_pool.Close();
        CaptureError::SystemFailure("timed out waiting for the first capture frame".into())
    })?;

    let _ = frame_pool.RemoveFrameArrived(token);
    let _ = session.Close();
    let _ = frame_pool.Close();

    let frame = frame_result.map_err(map_capture_error)?;
    let surface = frame.Surface().map_err(map_capture_error)?;
    let bitmap = wait_async_operation(
        SoftwareBitmap::CreateCopyWithAlphaFromSurfaceAsync(&surface, BitmapAlphaMode::Ignore)
            .map_err(map_capture_error)?,
    )
    .map_err(map_capture_error)?;
    let _ = frame.Close();
    Ok(bitmap)
}

fn create_direct3d_device() -> Result<IDirect3DDevice, CaptureError> {
    let mut device = None;
    let feature_levels = [D3D_FEATURE_LEVEL(0xb100), D3D_FEATURE_LEVEL(0xb000), D3D_FEATURE_LEVEL(0xa100)];
    unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&feature_levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
    }
    .map_err(map_capture_error)?;
    let device = device.ok_or_else(|| {
        CaptureError::SystemFailure("d3d11 device creation returned no device".into())
    })?;
    let dxgi_device: IDXGIDevice = device.cast().map_err(map_capture_error)?;
    let inspectable: IInspectable =
        unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }.map_err(map_capture_error)?;
    inspectable.cast().map_err(map_capture_error)
}

fn capture_item_for_monitor(monitor: HMONITOR) -> Result<GraphicsCaptureItem, CaptureError> {
    let interop: IGraphicsCaptureItemInterop = windows_core::factory::<
        GraphicsCaptureItem,
        IGraphicsCaptureItemInterop,
    >()
    .map_err(map_capture_error)?;
    unsafe { interop.CreateForMonitor(monitor) }.map_err(map_capture_error)
}

fn monitor_for_display_id(display_id: DisplayId) -> Result<HMONITOR, CaptureError> {
    available_monitors()
        .into_iter()
        .nth(display_id.0 as usize)
        .ok_or(CaptureError::InvalidInput("display id was not found"))
}

fn available_monitors() -> Vec<HMONITOR> {
    let mut monitors = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_enum_proc),
            LPARAM((&mut monitors as *mut Vec<HMONITOR>) as isize),
        );
    }
    monitors
}

unsafe extern "system" fn monitor_enum_proc(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let monitors = data.0 as *mut Vec<HMONITOR>;
    unsafe {
        (*monitors).push(monitor);
    }
    BOOL(1)
}

#[allow(dead_code)]
fn monitor_info(monitor: HMONITOR) -> Result<MONITORINFOEXW, CaptureError> {
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    unsafe {
        GetMonitorInfoW(
            monitor,
            &mut info as *mut MONITORINFOEXW as *mut MONITORINFO,
        )
    }
    .ok()
    .map_err(|err| CaptureError::SystemFailure(format!("failed to query monitor info: {err}")))?;
    Ok(info)
}

fn map_capture_error(err: windows_core::Error) -> CaptureError {
    if err.code() == E_ACCESSDENIED {
        CaptureError::PermissionDenied
    } else {
        CaptureError::SystemFailure(err.to_string())
    }
}
