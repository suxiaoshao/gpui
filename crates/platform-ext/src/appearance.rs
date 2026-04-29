#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemAccentColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl SystemAccentColor {
    pub fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.red, self.green, self.blue)
    }
}

pub struct SystemAccentColorObserver {
    _inner: ObserverInner,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl SystemAccentColorObserver {
    fn new(inner: ObserverInner) -> Self {
        Self { _inner: inner }
    }
}

#[cfg(target_os = "macos")]
enum ObserverInner {
    Mac { _observer: macos::MacObserver },
}

#[cfg(target_os = "windows")]
enum ObserverInner {
    Windows { _observer: windows::WindowsObserver },
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
enum ObserverInner {}

#[cfg(target_os = "macos")]
mod macos {
    use super::{ObserverInner, SystemAccentColor, SystemAccentColorObserver};
    use block2::RcBlock;
    use objc2::{
        rc::Retained,
        runtime::{AnyObject, NSObjectProtocol, ProtocolObject},
    };
    use objc2_app_kit::{NSColor, NSColorSpace, NSSystemColorsDidChangeNotification};
    use objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue};
    use std::{ptr::NonNull, sync::Arc};

    pub(super) struct MacObserver {
        center: Retained<NSNotificationCenter>,
        token: Retained<ProtocolObject<dyn NSObjectProtocol>>,
        _block: RcBlock<dyn Fn(NonNull<NSNotification>) + 'static>,
    }

    impl Drop for MacObserver {
        fn drop(&mut self) {
            let token: &ProtocolObject<dyn NSObjectProtocol> = &self.token;
            let token: &AnyObject = token.as_ref();
            unsafe {
                self.center.removeObserver(token);
            }
        }
    }

    pub(super) fn system_accent_color() -> Option<SystemAccentColor> {
        let color = NSColor::controlAccentColor();
        let color_space = NSColorSpace::sRGBColorSpace();
        let color = color.colorUsingColorSpace(&color_space)?;
        Some(SystemAccentColor::new(
            component_to_u8(color.redComponent()),
            component_to_u8(color.greenComponent()),
            component_to_u8(color.blueComponent()),
        ))
    }

    pub(super) fn observe_system_accent_color_changes(
        callback: impl Fn() + Send + Sync + 'static,
    ) -> Option<SystemAccentColorObserver> {
        let callback: Arc<dyn Fn() + Send + Sync> = Arc::new(callback);
        let block_callback = Arc::clone(&callback);
        let block: RcBlock<dyn Fn(NonNull<NSNotification>) + 'static> =
            RcBlock::new(move |_notification| {
                block_callback();
            });

        let center = NSNotificationCenter::defaultCenter();
        let queue = NSOperationQueue::mainQueue();
        let token = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSSystemColorsDidChangeNotification),
                None,
                Some(&queue),
                &block,
            )
        };

        Some(SystemAccentColorObserver::new(ObserverInner::Mac {
            _observer: MacObserver {
                center,
                token,
                _block: block,
            },
        }))
    }

    fn component_to_u8(component: f64) -> u8 {
        (component.clamp(0.0, 1.0) * 255.0).round() as u8
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{ObserverInner, SystemAccentColor, SystemAccentColorObserver};
    use std::sync::Arc;
    use windows::{
        Foundation::TypedEventHandler,
        UI::ViewManagement::{UIColorType, UISettings},
    };
    use windows_core::IInspectable;

    pub(super) struct WindowsObserver {
        settings: UISettings,
        token: i64,
        _handler: TypedEventHandler<UISettings, IInspectable>,
    }

    impl Drop for WindowsObserver {
        fn drop(&mut self) {
            let _ = self.settings.RemoveColorValuesChanged(self.token);
        }
    }

    pub(super) fn system_accent_color() -> Option<SystemAccentColor> {
        let settings = UISettings::new().ok()?;
        let color = settings.GetColorValue(UIColorType::Accent).ok()?;
        Some(SystemAccentColor::new(color.R, color.G, color.B))
    }

    pub(super) fn observe_system_accent_color_changes(
        callback: impl Fn() + Send + Sync + 'static,
    ) -> Option<SystemAccentColorObserver> {
        let callback: Arc<dyn Fn() + Send + Sync> = Arc::new(callback);
        let handler_callback = Arc::clone(&callback);
        let handler = TypedEventHandler::<UISettings, IInspectable>::new(move |_, _| {
            handler_callback();
            Ok(())
        });
        let settings = UISettings::new().ok()?;
        let token = settings.ColorValuesChanged(&handler).ok()?;

        Some(SystemAccentColorObserver::new(ObserverInner::Windows {
            _observer: WindowsObserver {
                settings,
                token,
                _handler: handler,
            },
        }))
    }
}

pub fn system_accent_color() -> Option<SystemAccentColor> {
    #[cfg(target_os = "macos")]
    {
        macos::system_accent_color()
    }

    #[cfg(target_os = "windows")]
    {
        windows::system_accent_color()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

pub fn observe_system_accent_color_changes(
    callback: impl Fn() + Send + Sync + 'static,
) -> Option<SystemAccentColorObserver> {
    #[cfg(target_os = "macos")]
    {
        macos::observe_system_accent_color_changes(callback)
    }

    #[cfg(target_os = "windows")]
    {
        windows::observe_system_accent_color_changes(callback)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = callback;
        None
    }
}
