use compose::desktop::{WindowOptions, run_app};
use compose::ui::LayoutDirection;

fn main() {
    // `GALLERY_ABOUT=1` opens the "About" dialog from the start; `GALLERY_RTL=1` lays the
    // gallery out right to left; `GALLERY_SCREEN=drawing` (or `fields`) shows that check screen
    // instead (android-demo's tabs of those names), to compare a platform's with the desktop's.
    let layout_direction = if std::env::var_os("GALLERY_RTL").is_some() {
        LayoutDirection::Rtl
    } else {
        LayoutDirection::Ltr
    };
    let screen = std::env::var("GALLERY_SCREEN").unwrap_or_default();
    let size = if screen.is_empty() { (420, 760) } else { (840, 1400) };
    run_app(
        WindowOptions {
            title: "Gallery".into(),
            size: size.into(),
            layout_direction,
            ..Default::default()
        },
        move || match screen.as_str() {
            "drawing" => gallery::Drawing(),
            "fields" => gallery::Fields(),
            _ => gallery::GalleryWith(std::env::var_os("GALLERY_ABOUT").is_some()),
        },
    );
}
