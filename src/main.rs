use x11rb::atom_manager;
use x11rb::connection::Connection;
use x11rb::errors::{ReplyError};
use x11rb::protocol::render::{self, ConnectionExt as _, PictType};
use x11rb::protocol::xproto::{ConnectionExt as _, *};
use x11rb::xcb_ffi::XCBConnection;
use std::{fs, path::{Path, PathBuf}};

atom_manager! {
    pub AtomCollection: AtomCollectionCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
        _NET_WM_NAME,
        UTF8_STRING,
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct xcb_visualtype_t {
    pub visual_id: u32,
    pub class: u8,
    pub bits_per_rgb_value: u8,
    pub colormap_entries: u16,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    pub pad0: [u8; 4],
}

impl From<Visualtype> for xcb_visualtype_t {
    fn from(value: Visualtype) -> xcb_visualtype_t {
        xcb_visualtype_t {
            visual_id: value.visual_id,
            class: value.class.into(),
            bits_per_rgb_value: value.bits_per_rgb_value,
            colormap_entries: value.colormap_entries,
            red_mask: value.red_mask,
            green_mask: value.green_mask,
            blue_mask: value.blue_mask,
            pad0: [0; 4],
        }
    }
}

fn choose_visual(conn: &impl Connection, screen_num: usize) -> Result<(u8, Visualid), ReplyError> {
    let depth = 24;
    let screen = &conn.setup().roots[screen_num];

    // Try to use XRender to find a visual with alpha support
    let has_render = conn
        .extension_information(render::X11_EXTENSION_NAME)?
        .is_some();

    if has_render {
        let formats = conn.render_query_pict_formats()?.reply()?;
        // Find the ARGB32 format that must be supported.
        let format = formats
            .formats
            .iter()
            .filter(|info| (info.type_, info.depth) == (PictType::DIRECT, depth))
            .filter(|info| {
                let d = info.direct;
                (d.red_mask, d.green_mask, d.blue_mask, d.alpha_mask) == (0xff, 0xff, 0xff, 0xff)
            })
            .find(|info| {
                let d = info.direct;
                (d.red_shift, d.green_shift, d.blue_shift, d.alpha_shift) == (16, 8, 0, 24)
            });

        if let Some(format) = format {
            if let Some(visual) = formats.screens[screen_num]
                .depths
                .iter()
                .flat_map(|d| &d.visuals)
                .find(|v| v.format == format.id)
            {
                return Ok((format.depth, visual.visual));
            }
        }
    }
    Ok((screen.root_depth, screen.root_visual))
}

/// Check if a composite manager is running
fn composite_manager_running(
    conn: &impl Connection,
    screen_num: usize,
) -> Result<bool, ReplyError> {
    let atom = format!("_NET_WM_CM_S{}", screen_num);
    let atom = conn.intern_atom(false, atom.as_bytes())?.reply()?.atom;
    let owner = conn.get_selection_owner(atom)?.reply()?;
    Ok(owner.owner != x11rb::NONE)
}

fn read_bitmap_files(directory_path: &str) -> Vec<PathBuf> {
    let dir = match fs::read_dir(directory_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Err reading directory: {}", e);
            return Vec::new();
        }
    };

    let bitmap_files: Vec<PathBuf> = dir
        .filter_map(|entry| {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "bmp") {
                    Some(path)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    bitmap_files
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let image_dir = "/home/eatmynerds/repos/paperview/cyberpunk-bmp";

    println!("Loading images");

    let bitmap_files = read_bitmap_files(image_dir);

    println!("Loading monitors");

    let (conn, current_screen) = XCBConnection::connect(None)?;
    let screen = &conn.setup().roots[current_screen];
    let atoms = AtomCollection::new(&conn)?.reply()?;
    let (mut width, mut height) = (3840, 1080);
    let (depth, visualid) = choose_visual(&conn, current_screen)?;

    println!("Screen: {}, width: {}, height: {}, depth: {}", current_screen, width, height, depth);

    let transparency = composite_manager_running(&conn, current_screen)?;
    println!(
        "Composite manager running / working transparency: {:?}",
        transparency
    );

    Ok(())
}
