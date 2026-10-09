//! Static catalog of the Crafting Apps, most of which are listed on <https://getartcraft.com/apps>.

use eframe::egui::Color32;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    EarlyAlpha,
    InDevelopment,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Stage::EarlyAlpha => "Early alpha",
            Stage::InDevelopment => "In development",
        }
    }
}

pub struct Highlight {
    pub title: &'static str,
    pub body: &'static str,
}

pub struct CraftApp {
    /// Lowercase identifier; also the binary name, repo name and asset name.
    pub id: &'static str,
    /// The part of the name before "Craft", e.g. "Photo".
    pub prefix: &'static str,
    pub category: &'static str,
    pub tagline: &'static str,
    pub summary: &'static str,
    /// Background colour of the app icon, used as the app's accent everywhere.
    pub accent: Color32,
    pub stage: Stage,
    pub version: &'static str,
    pub web: bool,
    /// Whether the app has its own page on getartcraft.com yet.
    pub listed: bool,
    pub icon_webp: &'static [u8],
    pub highlights: [Highlight; 6],
}

impl CraftApp {
    pub fn name(&self) -> String {
        format!("{}Craft", self.prefix)
    }

    pub fn website(&self) -> Option<String> {
        self.listed.then(|| format!("https://getartcraft.com/apps/{}", self.id))
    }

    pub fn repo(&self) -> String {
        format!("https://github.com/storytold/{}", self.id)
    }

    pub fn releases(&self) -> String {
        format!("https://github.com/storytold/{}/releases/latest", self.id)
    }

    pub fn build_command(&self) -> String {
        format!(
            "git clone https://github.com/storytold/{id}.git\ncd {id}\ncargo run --release -p {id}",
            id = self.id
        )
    }
}

macro_rules! icon {
    ($id:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/", $id, ".webp"))
    };
}

const fn h(title: &'static str, body: &'static str) -> Highlight {
    Highlight { title, body }
}

pub const DISCORD: &str = "https://discord.gg/artcraft";
pub const GITHUB_ORG: &str = "https://github.com/storytold";
pub const APPS_PAGE: &str = "https://getartcraft.com/apps";

pub static APPS: [CraftApp; 9] = [
    CraftApp {
        id: "photocraft",
        prefix: "Photo",
        category: "Image editor",
        tagline: "The open-source image editor you already know how to use.",
        summary: "Layers, masks, adjustment layers, layer styles, type, vectors and brushes in a \
                  native app written entirely in Rust. Open source, offline, and yours.",
        accent: Color32::from_rgb(0x2f, 0x7b, 0xf5),
        stage: Stage::EarlyAlpha,
        version: "0.5.0",
        web: true,
        listed: true,
        icon_webp: icon!("photocraft"),
        highlights: [
            h("Familiar by design", "The menus, shortcuts, panels and tools your hands already know. Productive on day one."),
            h("Native GPU canvas", "Renders on Metal, Vulkan, DirectX 12 and WebGPU. No Electron, no web view."),
            h("Real layered files", "Opens and saves layered PSD and PSB documents that round-trip faithfully."),
            h("Non-destructive editing", "16 adjustment layers, 70+ live-preview filters, smart objects and layer styles."),
            h("On-device selection", "Select subjects and refine masks with tools that run on your machine."),
            h("Serious color", "8, 16 and 32-bit documents in RGB, CMYK, Lab and Grayscale with ICC management."),
        ],
    },
    CraftApp {
        id: "vectorcraft",
        prefix: "Vector",
        category: "Vector illustration",
        tagline: "Fast, open-source vector illustration, reimagined in pure Rust.",
        summary: "The pen, panels and shortcuts working illustrators expect, in a native app that \
                  keeps up with you. Open source, and in your browser too.",
        accent: Color32::from_rgb(0xe7, 0x57, 0x3e),
        stage: Stage::InDevelopment,
        version: "0.6.0",
        web: true,
        listed: true,
        icon_webp: icon!("vectorcraft"),
        highlights: [
            h("The workflow you know", "Pen, direct selection, shape booleans, snapping guides and an appearance panel."),
            h("Genuinely fast", "20,000 shapes render in about 27 ms at retina resolution; the UI runs at 120 fps."),
            h("Booleans that never fail", "Exact curve booleans and unlimited undo."),
            h("Live, editable effects", "Blends, envelope distort, gradient mesh, repeats, glows and shadows."),
            h("Image tracing", "Turn raster images into clean vector paths with 12 tracing presets."),
            h("Open formats", "Native JSON documents, SVG and PDF in and out, plus PNG, JPEG and WebP export."),
        ],
    },
    CraftApp {
        id: "filmcraft",
        prefix: "Film",
        category: "Video editor",
        tagline: "Professional, open-source video editing, rebuilt from scratch in Rust.",
        summary: "Source and program monitors, a real timeline and every trim mode, in a native \
                  editor written in pure Rust — down to its own codecs.",
        accent: Color32::from_rgb(0x8b, 0x5b, 0xf6),
        stage: Stage::InDevelopment,
        version: "0.4.0",
        web: false,
        listed: true,
        icon_webp: icon!("filmcraft"),
        highlights: [
            h("A timeline you already know", "Source and program monitors, three-point editing, every trim mode, J/K/L."),
            h("Frame-exact time", "An integer timebase keeps 23.976 and 29.97 drop-frame exact. No drift."),
            h("Its own codecs", "No FFmpeg inside. Native H.264, HEVC, ProRes, VP9, AAC and Opus."),
            h("Color grading", "Curves, color wheels, HSL secondaries, LUTs, HDR and log workflows, plus scopes."),
            h("Effects and keyframes", "About 55 effects and 30 transitions on a linear-light GPU compositor."),
            h("Broadcast-ready audio", "EBU R128 loudness meters, a mixer with automation and native DSP."),
        ],
    },
    CraftApp {
        id: "lightcraft",
        prefix: "Light",
        category: "Photo library & raw developer",
        tagline: "An open-source photo library and raw developer that runs on your machine.",
        summary: "A fast photo library and non-destructive raw developer, written from scratch in \
                  pure Rust. No account, no cloud, no telemetry, no subscription.",
        accent: Color32::from_rgb(0xf3, 0xa6, 0x17),
        stage: Stage::InDevelopment,
        version: "0.4.0",
        web: true,
        listed: true,
        icon_webp: icon!("lightcraft"),
        highlights: [
            h("Non-destructive by design", "A scene-referred, wide-gamut, 32-bit float pipeline. Originals untouched."),
            h("Every control you reach for", "Tone curve, 8-band color mixer, 3-way grading wheels and 18 presets."),
            h("Precise masking", "Brush, gradients, luminance and color ranges, plus sky and subject masks."),
            h("Native raw decoding", "Pure-Rust decoders for DNG, CR2, ARW, NEF, RAF, RW2, PEF and ORF."),
            h("A library that keeps up", "Albums, ratings, flags, field search and virtualized grids for big catalogs."),
            h("Instant feedback", "The GPU develop pipeline updates a 24 MP raw in about 4 ms per slider move."),
        ],
    },
    CraftApp {
        id: "pdfcraft",
        prefix: "Pdf",
        category: "PDF workbench",
        tagline: "The open-source PDF workbench: read, organize, combine, split and secure.",
        summary: "Read, organize, combine, split and secure PDFs in a fast, native app written in \
                  Rust from the ground up. No account, no telemetry, no cloud.",
        accent: Color32::from_rgb(0x13, 0xa4, 0x8a),
        stage: Stage::EarlyAlpha,
        version: "0.4.0",
        web: true,
        listed: true,
        icon_webp: icon!("pdfcraft"),
        highlights: [
            h("Faithful rendering", "World scripts, vertical Japanese, color emoji and transparency."),
            h("Search as you type", "Instant find, with selection that follows the real reading order."),
            h("Organize like cards", "Rotate, delete, insert and reorder pages, with deep undo."),
            h("Combine, extract, split", "Keeps links, form fields, layers and bookmarks intact."),
            h("Fearless saves", "Incremental, atomic saves with autosave and crash recovery."),
            h("Built-in security", "Opens everything from RC4 to AES-256 and honors author permissions."),
        ],
    },
    CraftApp {
        id: "effectcraft",
        prefix: "Effect",
        category: "Motion graphics & VFX",
        tagline: "Open-source motion graphics and visual effects, built in pure Rust.",
        summary: "Compositions, layers, keyframes, 259 effects, expressions, 3D cameras and lights, \
                  and a render queue in a native compositor written in pure Rust.",
        accent: Color32::from_rgb(0xe0, 0x36, 0x90),
        stage: Stage::InDevelopment,
        version: "0.6.0",
        web: true,
        listed: true,
        icon_webp: icon!("effectcraft"),
        highlights: [
            h("Animate the way you know", "Linear, bezier, hold and eased keys, roving keys and a graph editor."),
            h("Layers of every kind", "Shapes, text, footage, precomps, nulls, cameras and lights, 38 blend modes."),
            h("259 effects", "Blur, color, distortion, generators, keying, particles, stylize and time."),
            h("Real 3D", "Cameras, depth of field and lights casting soft ray-traced shadows."),
            h("Expressions", "JavaScript expressions with wiggle, loops and layer references."),
            h("Export anywhere", "H.264 and ProRes with alpha, image sequences, GIF and Lottie."),
        ],
    },
    CraftApp {
        id: "designcraft",
        prefix: "Design",
        category: "Page layout & publishing",
        tagline: "Open-source page layout and publishing, rebuilt in pure Rust.",
        summary: "Spreads and parent pages, threaded stories, styles, swatches and text wrap in a \
                  native layout app with a professional paragraph composer.",
        accent: Color32::from_rgb(0x7a, 0xb5, 0x1b),
        stage: Stage::InDevelopment,
        version: "0.4.0",
        web: true,
        listed: true,
        icon_webp: icon!("designcraft"),
        highlights: [
            h("The layout tools you know", "Spreads, parent pages, threaded frames, styles, swatches and text wrap."),
            h("Beautiful type", "A Knuth–Plass composer, hyphenation, optical margins and baseline grids."),
            h("Print-ready PDF", "Embedded fonts, CMYK and spot colors, bleed, crop marks and PDF/A."),
            h("Fast", "Multithreaded SIMD rendering and copy-on-write documents with instant undo."),
            h("Open formats", "A documented native format, PNG export and layout interchange."),
            h("Agent-native", "Every menu, tool and dialog can be driven over JSON control or MCP."),
        ],
    },
    CraftApp {
        id: "soundcraft",
        prefix: "Sound",
        category: "Audio workstation",
        tagline: "Open-source recording, editing and mixing, rebuilt in pure Rust.",
        summary: "A complete digital audio workstation: multitrack editing, a full mixer with plugins, \
                  sends and automation, MIDI, recording and bouncing, in a native app written in pure Rust.",
        accent: Color32::from_rgb(0x14, 0xa9, 0xc4),
        stage: Stage::InDevelopment,
        version: "0.3.0",
        web: true,
        listed: false,
        icon_webp: icon!("soundcraft"),
        highlights: [
            h("The workflow you know", "Edit and Mix windows, Shuffle, Slip, Spot and Grid modes, playlists and memory locations."),
            h("A full mixer", "Ten inserts and ten sends per track, busses, VCAs and automatic delay compensation."),
            h("Live automation", "Volume, pan, sends and every plugin parameter, written in Write, Touch, Latch and Trim."),
            h("MIDI and notation", "Built-in synths, a piano roll with velocity lane, step input, quantize and a Score Editor."),
            h("Your plugins too", "Original EQs, dynamics, reverbs and delays, plus CLAP, VST3 and Audio Units."),
            h("Surround and picture", "Formats up to 9.1.6 and Ambisonics, and a video track decoding H.264 and ProRes."),
        ],
    },
    CraftApp {
        id: "cadcraft",
        prefix: "CAD",
        category: "CAD & drafting",
        tagline: "Open-source computer-aided design and drafting, rebuilt in pure Rust.",
        summary: "The command line, object snaps, layers, dimensions, hatches, blocks and DXF drawings \
                  you already know, in a fast native app written in pure Rust.",
        accent: Color32::from_rgb(0x14, 0xa3, 0xc7),
        stage: Stage::InDevelopment,
        version: "0.3.0",
        web: true,
        listed: false,
        icon_webp: icon!("cadcraft"),
        highlights: [
            h("The workflow you know", "Type L, click two points, type @5<45. Prompts, AutoComplete, grips and repeat."),
            h("Precision drafting", "Object snaps, polar tracking, ortho, grid snap and direct distance entry."),
            h("Full annotation", "Every DIM command with dimension styles, multileaders, tables and TrueType text."),
            h("Open drawings", "DXF read and write from R12 to 2018, and DWG through the open-source acadrust."),
            h("Layouts and plotting", "Paper space, scaled viewports, page setups and PLOT to PDF, SVG and PNG."),
            h("Parametric", "Geometric and dimensional constraints that re-solve after every edit."),
        ],
    },
];
