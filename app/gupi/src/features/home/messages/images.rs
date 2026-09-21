//! Compact user-message thumbnails; the Pi message remains the image authority.
use crate::foundation::i18n::t;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use gpui_kit::{
    component::{
        ActiveTheme,
        button::{Button, ButtonVariants},
    },
    *,
};
use std::{io::Cursor, sync::Arc};

#[derive(IntoElement)]
pub(super) struct UserImage {
    pub preview_host: Entity<super::super::image_preview::PreviewHost>,
    pub id: String,
    pub mime: String,
    pub data: String,
}

struct Thumbnail {
    image: Arc<Image>,
    dimensions: (u32, u32),
}
struct ImageState {
    mime: String,
    data: String,
    thumbnail: Option<Thumbnail>,
}

impl UserImage {
    fn decode(&self) -> Option<Thumbnail> {
        let format = ImageFormat::from_mime_type(&self.mime)?;
        let bytes = STANDARD.decode(&self.data).ok()?;
        let dimensions = image::ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()
            .ok()?
            .into_dimensions()
            .ok()?;
        if dimensions.0 == 0 || dimensions.1 == 0 {
            return None;
        }
        Some(Thumbnail {
            image: Arc::new(Image::from_bytes(format, bytes)),
            dimensions,
        })
    }
}

impl RenderOnce for UserImage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let preview_host = self.preview_host.clone();
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| ImageState {
            mime: self.mime.clone(),
            data: self.data.clone(),
            thumbnail: self.decode(),
        });
        state.update(cx, |state, _| {
            if state.mime != self.mime || state.data != self.data {
                state.thumbnail = self.decode();
                state.mime = self.mime;
                state.data = self.data;
            }
        });
        let unavailable = t(cx, "tool-detail-image-unavailable");
        let Some(Thumbnail { image, dimensions }) = &state.read(cx).thumbnail else {
            return div().text_sm().child(unavailable).into_any_element();
        };
        let dimensions = *dimensions;
        let (width, height) = (dimensions.0 as f32, dimensions.1 as f32);
        // Reserve the actual aspect ratio before pixels decode, keeping virtual rows stable.
        let scale = (12. / width).min(10. / height);
        let image = image.clone();
        let preview = image.clone();
        let label = t(cx, "image-preview-open");
        Button::new(id)
            .ghost()
            .p_0()
            .w(rems(width * scale))
            .h(rems(height * scale))
            .rounded(cx.theme().radius_lg)
            .overflow_hidden()
            .accessibility_label(label.clone())
            .tooltip(label.clone())
            .child(
                img(image)
                    .size_full()
                    .rounded(cx.theme().radius_lg)
                    .object_fit(ObjectFit::Contain)
                    .with_fallback(move || div().child(unavailable.clone()).into_any_element()),
            )
            .on_click(move |_, window, cx| {
                super::super::image_preview::open_message(
                    &preview_host,
                    preview.clone(),
                    dimensions,
                    window,
                    cx,
                );
            })
            .into_any_element()
    }
}
