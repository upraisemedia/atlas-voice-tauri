//! General pasteboard snapshot, write and restore.

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSPasteboardWriting};
use objc2_foundation::{NSArray, NSData, NSString};

/// Every item on the pasteboard with all of its representations, copied into
/// plain Rust data so it can cross threads. Keeps images, files and rich text.
pub type Snapshot = Vec<Vec<(String, Vec<u8>)>>;

/// Markers from nspasteboard.org: clipboard managers (Alfred, Raycast, Paste,
/// Maccy...) do not record items carrying them.
const PRIVACY_MARKERS: [&str; 2] = [
    "org.nspasteboard.TransientType",
    "org.nspasteboard.ConcealedType",
];

pub fn change_count() -> isize {
    NSPasteboard::generalPasteboard().changeCount()
}

pub fn snapshot() -> Snapshot {
    let pasteboard = NSPasteboard::generalPasteboard();
    let Some(items) = pasteboard.pasteboardItems() else {
        return Vec::new();
    };
    items
        .iter()
        .map(|item| {
            item.types()
                .iter()
                .filter_map(|ty| {
                    item.dataForType(&ty)
                        .map(|data| (ty.to_string(), data.to_vec()))
                })
                .collect()
        })
        .collect()
}

/// Replaces the pasteboard with `text` and returns the new change count.
pub fn write_text(text: &str) -> isize {
    let item = NSPasteboardItem::new();
    item.setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString });
    for marker in PRIVACY_MARKERS {
        item.setData_forType(&NSData::new(), &NSString::from_str(marker));
    }
    write_items(vec![item])
}

pub fn restore(snapshot: Snapshot) {
    let items = snapshot
        .into_iter()
        .map(|representations| {
            let item = NSPasteboardItem::new();
            for (ty, bytes) in representations {
                item.setData_forType(&NSData::with_bytes(&bytes), &NSString::from_str(&ty));
            }
            item
        })
        .collect();
    write_items(items);
}

fn write_items(items: Vec<Retained<NSPasteboardItem>>) -> isize {
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    if !items.is_empty() {
        let writers: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = items
            .into_iter()
            .map(ProtocolObject::from_retained)
            .collect();
        pasteboard.writeObjects(&NSArray::from_retained_slice(&writers));
    }
    pasteboard.changeCount()
}
