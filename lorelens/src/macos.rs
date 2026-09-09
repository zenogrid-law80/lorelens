use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSImage};
use objc2_foundation::NSData;

pub fn set_application_icon() {
  let mtm = MainThreadMarker::new().expect("GPUI starts on the main thread");
  let data = NSData::with_bytes(include_bytes!("../dist/favicon.ico"));
  let image = NSImage::initWithData(NSImage::alloc(), &data);
  if let Some(image) = image {
    // SAFETY: A valid image is supplied, and AppKit runs on the main thread.
    unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
  }
}
