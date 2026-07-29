use super::*;

#[link(name = "QuickLookUI", kind = "framework")]
unsafe extern "C" {}

thread_local! {
    static CURRENT_URL: RefCell<Option<Retained<NSURL>>> = const { RefCell::new(None) };
    static DATA_SOURCE: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) };
}

pub(super) fn preview_file(path: &Path) -> Result<(), WindowExtError> {
    let _ = MainThreadMarker::new().ok_or(WindowExtError::QuickLookRequiresMainThread)?;
    let url = NSURL::from_file_path(path).ok_or(WindowExtError::FailedToCreateQuickLookUrl)?;
    CURRENT_URL.with(|current_url| {
        current_url.replace(Some(url));
    });
    let data_source = data_source_object()?;
    let panel_class =
        AnyClass::get(c"QLPreviewPanel").ok_or(WindowExtError::FailedToGetQuickLookPanel)?;
    let panel: *mut AnyObject = unsafe { msg_send![panel_class, sharedPreviewPanel] };
    let Some(panel) = (unsafe { panel.as_ref() }) else {
        return Err(WindowExtError::FailedToGetQuickLookPanel);
    };
    let _: () = unsafe { msg_send![panel, setDataSource: &*data_source] };
    let _: () = unsafe { msg_send![panel, reloadData] };
    let _: () = unsafe { msg_send![panel, makeKeyAndOrderFront: ptr::null_mut::<AnyObject>()] };
    Ok(())
}

fn data_source_object() -> Result<Retained<AnyObject>, WindowExtError> {
    DATA_SOURCE.with(|slot| {
        if let Some(data_source) = slot.borrow().as_ref() {
            return Ok(data_source.clone());
        }
        let class = data_source_class();
        let data_source: Retained<AnyObject> = unsafe { msg_send![class, new] };
        slot.replace(Some(data_source.clone()));
        Ok(data_source)
    })
}

fn data_source_class() -> &'static AnyClass {
    static CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
    CLASS.get_or_init(|| {
        extern "C-unwind" fn number_of_items(
            _this: &AnyObject,
            _sel: Sel,
            _panel: *mut AnyObject,
        ) -> isize {
            CURRENT_URL.with(
                |current_url| {
                    if current_url.borrow().is_some() { 1 } else { 0 }
                },
            )
        }

        extern "C-unwind" fn item_at_index(
            _this: &AnyObject,
            _sel: Sel,
            _panel: *mut AnyObject,
            index: isize,
        ) -> *mut AnyObject {
            if index != 0 {
                return ptr::null_mut();
            }
            CURRENT_URL.with(|current_url| {
                current_url
                    .borrow()
                    .as_ref()
                    .map(|url| ptr::from_ref::<NSURL>(url).cast_mut().cast::<AnyObject>())
                    .unwrap_or_else(ptr::null_mut)
            })
        }

        let mut builder = ClassBuilder::new(c"WindowExtQuickLookDataSource", NSObject::class())
            .expect("WindowExtQuickLookDataSource class should register once");
        unsafe {
            builder.add_method(
                sel!(numberOfPreviewItemsInPreviewPanel:),
                number_of_items as extern "C-unwind" fn(_, _, _) -> _,
            );
            builder.add_method(
                sel!(previewPanel:previewItemAtIndex:),
                item_at_index as extern "C-unwind" fn(_, _, _, _) -> _,
            );
        }
        builder.register()
    })
}
