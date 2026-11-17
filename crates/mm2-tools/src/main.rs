//! `mm2-tools`: inspect and unpack Motocross Madness 2 data files.

use mm2_formats::{dtm, faoe, rs2, slb, tga, trn, Container};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

mod freetrack;
use mm2_course::free as freetracks;
mod terrain;
mod textures;

const USAGE: &str = "\
usage:
  mm2-tools info <file>...            show the container format of each file
  mm2-tools list <file>               list the entries in an archive
  mm2-tools extract <file> <out-dir>  write every entry of an archive to out-dir
  mm2-tools decode <file> <out-file>  write the RS2 archive inside a FAOE file
  mm2-tools terrain <track.env> <out-dir>  export a track's heightmap and texture map
  mm2-tools model <file.slb>          show a model's frames, materials and surfaces
  mm2-tools dialog <UI/X.DAT> [out.png]  show a front-end dialog's controls, and draw it
  mm2-tools strings <X.DLL>           list a DLL's string table (UILANG.DLL, LANG.DLL)
  mm2-tools overlay <file.tga> <out.png>  convert an overlay picture from RES/GLOBAL.RES
  mm2-tools textures <GAME-dir> <out-dir>  export every texture as PNG for upscaling
  mm2-tools freetrack <out.env> [seed]      make a new free test track from scratch
  mm2-tools freetracks <GAME-dir> [ground-dir] [only]  make the free pack's tracks (only: a file or event folder)
  mm2-tools preview <track.env> <out.png>  draw a track's preview picture from its ground
  mm2-tools course <in.course|mode|track.env> <out.env>  bake a Course Builder track (mode: sx, national, baja, enduro; an .env: a copy of it)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["info", ref files @ ..] if !files.is_empty() => files.iter().try_for_each(|f| info(f)),
        ["list", file] => list(file),
        ["extract", file, out] => extract(file, Path::new(out)),
        ["decode", file, out] => decode(file, Path::new(out)),
        ["terrain", file, out] => export_terrain(file, Path::new(out)),
        ["model", file] => model(file),
        ["textures", game, out] => textures::export(Path::new(game), Path::new(out)),
        ["dialog", file] => dialog(file, None),
        ["strings", file] => strings(file),
        ["overlay", file, out] => overlay(file, out),
        ["freetrack", out] => freetrack::build(Path::new(out), 1).map_err(|e| e.to_string().into()),
        ["freetrack", out, seed] => freetrack::build(Path::new(out), seed.parse().unwrap_or(1)).map_err(|e| e.to_string().into()),
        ["freetracks", game] => freetracks::build(Path::new(game), None, None).map_err(|e| e.to_string().into()),
        ["freetracks", game, ground] => freetracks::build(Path::new(game), Some(Path::new(ground)), None).map_err(|e| e.to_string().into()),
        ["freetracks", game, ground, only] => freetracks::build(Path::new(game), Some(Path::new(ground)).filter(|g| g.as_os_str() != "-"), Some(only)).map_err(|e| e.to_string().into()),
        ["dialog", file, out] => dialog(file, Some(out)),
        ["course", from, out] => course(from, Path::new(out)),
        ["preview", env, out] => preview(env, out),
        ["free-menus", ui, text, out] => free_menus(Path::new(ui), Path::new(text), Path::new(out)),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

type BoxResult = Result<(), Box<dyn std::error::Error>>;

/// Bakes a Course Builder file (or a new track of a mode) into an `.ENV`.
fn course(from: &str, out: &Path) -> BoxResult {
    use mm2_course::{Course, Mode};
    let c = match from.to_ascii_lowercase().as_str() {
        "sx" | "supercross" => Course::new(Mode::Supercross),
        "national" => Course::new(Mode::National),
        "baja" => Course::new(Mode::Baja),
        "enduro" => Course::new(Mode::Enduro),
        // A copy of a track, to change.
        f if f.ends_with(".env") => {
            let mut c = mm2_course::copy::import(&read(from)?, from, None, 0, "Copy")?;
            // MM2_COPY_JUMP=1: a tabletop half way round, to see it work.
            if std::env::var_os("MM2_COPY_JUMP").is_some() {
                let at = c.path().length / 2.0;
                c.jumps.push(mm2_course::kit::Kind::Tabletop.make(c.mode, at));
            }
            c
        }
        _ => {
            let mut c = Course::from_json(&String::from_utf8_lossy(&read(from)?))?;
            // A copy: read the track it was copied from.
            if let Some(base) = c.base.clone() {
                mm2_course::copy::attach(&mut c, &read(&base.file)?)?;
            }
            c
        }
    };
    let t = std::time::Instant::now();
    // MM2_GROUND=<dir>: paint the ground from <dir>/<material>.png, as
    // the builder does from the free pack's ground pictures.
    let mut pictures = mm2_course::env::Pictures::default();
    if let Some(dir) = std::env::var_os("MM2_GROUND") {
        for (name, _) in mm2_course::env::GROUND {
            let Ok(bytes) = std::fs::read(Path::new(&dir).join(format!("{name}.png"))) else { continue };
            let (w, h, rgb) = freetracks::decode_png(&bytes)?;
            if w != h {
                continue;
            }
            let rgb = rgb.chunks_exact(3).map(|p| [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0]).collect();
            pictures.by_name.insert(name.to_string(), std::sync::Arc::new(mm2_course::env::Picture { side: w, rgb }));
        }
    }
    let baked = mm2_course::env::write_with(&c, &pictures);
    std::fs::write(out, &baked.env)?;
    eprintln!("{} {}: {:.1} MB in {:.1} s, {} ft of line, {} gates", c.mode.name(), out.display(), baked.env.len() as f32 / 1e6, t.elapsed().as_secs_f32(), baked.path.length as u32, baked.gates.len());
    Ok(())
}

fn read(path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    std::fs::read(path).map_err(|e| format!("{path}: {e}").into())
}

fn info(path: &str) -> BoxResult {
    let bytes = read(path)?;
    let kind = Container::detect(&bytes);
    if kind == Container::Unknown {
        println!("{path}: unknown, {} bytes", bytes.len());
        return Ok(());
    }
    let inner = archive_bytes(path)?;
    let ar = rs2::Archive::parse(&inner)?;
    println!(
        "{path}: {kind}, {} entries, {} bytes",
        ar.entries().len(),
        bytes.len()
    );
    Ok(())
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Returns the archive bytes, decoding the outer FAOE layer if present.
fn archive_bytes(path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let bytes = read(path)?;
    match Container::detect(&bytes) {
        Container::Rs2 => Ok(bytes),
        Container::Faoe => Ok(faoe::decode(&bytes, &file_name(path))?),
        Container::Unknown => Err(format!("{path}: not an RS2 or FAOE file").into()),
    }
}

fn list(path: &str) -> BoxResult {
    let bytes = archive_bytes(path)?;
    let ar = rs2::Archive::parse(&bytes)?;
    for e in ar.entries() {
        println!("{:>10}  {:#010x}  {}", e.size, e.offset, e.name);
    }
    Ok(())
}

fn extract(path: &str, out: &Path) -> BoxResult {
    let bytes = archive_bytes(path)?;
    let ar = rs2::Archive::parse(&bytes)?;
    for e in ar.entries() {
        let dest = out.join(safe_relative(&e.name)?);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, ar.data(e))?;
    }
    println!(
        "extracted {} entries to {}",
        ar.entries().len(),
        out.display()
    );
    Ok(())
}

fn decode(path: &str, out: &Path) -> BoxResult {
    let bytes = read(path)?;
    let plain = faoe::decode(&bytes, &file_name(path))?;
    std::fs::write(out, plain)?;
    Ok(())
}

fn export_terrain(path: &str, out: &Path) -> BoxResult {
    let bytes = archive_bytes(path)?;
    let ar = rs2::Archive::parse(&bytes)?;
    let entry = ar
        .entries()
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(".trn"))
        .ok_or("no .trn entry in this archive")?;
    let t = trn::parse(ar.data(entry))?;
    let max = t.heights.iter().cloned().fold(f32::MIN, f32::max);
    println!(
        "{}: {}x{} samples, scale {}, max height {max:.2}, {} textures, {} tiles",
        entry.name,
        t.size,
        t.size,
        t.scale,
        t.textures.len(),
        t.tiles.len()
    );
    terrain::export(&t, out)?;
    println!("wrote heightmap.pgm and map.tga to {}", out.display());
    Ok(())
}

fn model(path: &str) -> BoxResult {
    let m = slb::parse(&read(path)?)?;
    let world = m.world_matrices();
    println!("{} frames", m.frames.len());
    for (i, f) in m.frames.iter().enumerate() {
        let parent = f.parent.map_or("-".to_string(), |p| p.to_string());
        let w = &world[i];
        println!(
            "  {i:>3} {:<24} parent {parent:>3}  at {:8.3} {:8.3} {:8.3}",
            f.name, w[12], w[13], w[14]
        );
    }
    println!("{} materials", m.materials.len());
    for (i, mat) in m.materials.iter().enumerate() {
        println!("  {i:>3} {}", mat.texture);
    }
    for (l, lod) in m.lods.iter().enumerate() {
        println!("LOD {l}: {} surfaces", lod.surfaces.len());
        for (i, s) in lod.surfaces.iter().enumerate() {
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in &s.positions {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            println!(
                "  {i:>3} {} verts, {} faces, materials {:?}, groups {:?}, bounds {lo:.2?}..{hi:.2?}",
                s.positions.len(),
                s.faces.len(),
                s.materials,
                s.groups
            );
        }
    }
    Ok(())
}

/// Turns an entry name into a relative path that cannot escape the output
/// directory.
fn safe_relative(name: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(name.replace('\\', "/"));
    if name.is_empty() || !path.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(format!("refusing unsafe entry name {name:?}"));
    }
    Ok(path)
}

/// Prints a front-end dialog's controls and draws it as it first appears.
fn dialog(path: &str, out: Option<&str>) -> BoxResult {
    let bytes = archive_bytes(path)?;
    let ar = rs2::Archive::parse(&bytes)?;
    let entry = ar
        .entries()
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(".dtm"))
        .ok_or("no .dtm in this archive")?;
    let d = dtm::parse(ar.data(entry));
    // Shared art lives in GLOBAL.DAT beside the dialogs.
    let global_bytes = Path::new(path)
        .parent()
        .map(|dir| dir.join("GLOBAL.DAT"))
        .and_then(|p| std::fs::read(p).ok())
        .unwrap_or_default();
    let global = rs2::Archive::parse(&global_bytes).ok();
    let global_dtm = global
        .as_ref()
        .and_then(|g| g.get("global.dtm"))
        .map(dtm::parse);
    println!(
        "{}: background {:?}, {} controls, {} images",
        entry.name,
        d.background(),
        d.controls.len(),
        d.anims.len()
    );
    let image = |name: &str| -> Option<tga::Image> {
        let file = d
            .anim(name)
            .or_else(|| global_dtm.as_ref()?.anim(name))
            .and_then(|a| a.frames.first().cloned())
            .unwrap_or_else(|| format!("{name}.tga"));
        let found = ar
            .get(&file)
            .or_else(|| global.as_ref()?.get(&file))
            .or_else(|| {
                let base = file.rsplit('\\').next()?;
                ar.entries()
                    .iter()
                    .find(|e| {
                        e.name
                            .rsplit('\\')
                            .next()
                            .is_some_and(|n| n.eq_ignore_ascii_case(base))
                    })
                    .map(|e| ar.data(e))
            });
        match found.map(tga::parse) {
            Some(Ok(img)) => Some(img),
            Some(Err(e)) => {
                println!("  {file}: {e}");
                None
            }
            None => {
                println!("  missing image {name} ({file})");
                None
            }
        }
    };
    for c in &d.controls {
        println!(
            "  {:<24} {:?} at {:?}{}{}",
            c.name,
            c.kind,
            d.place(c),
            c.anim("AnimNorm")
                .map_or(String::new(), |a| format!(" image {a}")),
            c.get("Text")
                .map_or(String::new(), |t| format!(" text {t:?}"))
        );
    }
    let Some(out) = out else { return Ok(()) };
    let (w, h) = (640u32, 480u32);
    let mut canvas = vec![0u8; (w * h * 4) as usize];
    let mut blit = |img: &tga::Image, x: i32, y: i32| {
        for sy in 0..img.height {
            for sx in 0..img.width {
                let (dx, dy) = (x + sx as i32, y + sy as i32);
                if dx < 0 || dy < 0 || dx >= w as i32 || dy >= h as i32 {
                    continue;
                }
                let p = img.pixel(sx, sy);
                if p[3] == 0 {
                    continue;
                }
                let i = ((dy as u32 * w + dx as u32) * 4) as usize;
                canvas[i..i + 4].copy_from_slice(&p);
            }
        }
    };
    if let Some(img) = d.background().and_then(&image) {
        blit(&img, 0, 0);
    }
    for c in &d.controls {
        if c.anchor.is_some() || c.props.flag("Show") == Some(false) {
            continue;
        }
        if let Some(img) = c.anim("AnimNorm").and_then(&image) {
            let r = d.place(c);
            blit(&img, r[0], r[1]);
        }
    }
    let file = std::fs::File::create(out)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&canvas)?;
    println!("wrote {out}");
    Ok(())
}

/// Draws a track's preview picture from its ground, as the builder does.
fn preview(env: &str, out: &str) -> BoxResult {
    let raw = read(env)?;
    let name = Path::new(env).file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let bytes = if raw.starts_with(b"RS2") || raw.starts_with(b"rs2") { raw } else { mm2_formats::faoe::decode(&raw, name).unwrap_or(raw) };
    let ar = mm2_formats::rs2::Archive::parse(&bytes)?;
    let files: Vec<(String, Vec<u8>)> = ar.entries().iter().map(|e| (e.name.clone(), ar.data(e).to_vec())).collect();
    let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let size = mm2_course::preview::LARGE;
    let rgb = mm2_course::preview::picture(stem, &files, size).ok_or("no terrain")?;
    let file = std::fs::File::create(out)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), size.0 as u32, size.1 as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&rgb)?;
    println!("wrote {out}");
    Ok(())
}

fn overlay(path: &str, out: &str) -> BoxResult {
    let img = mm2_formats::texture::parse_overlay(&read(path)?)?;
    let file = std::fs::File::create(out)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.width, img.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&img.rgba)?;
    println!("{}x{} wrote {out}", img.width, img.height);
    Ok(())
}

fn strings(path: &str) -> BoxResult {
    let table = mm2_formats::pe::strings(&read(path)?)?;
    let mut ids: Vec<_> = table.keys().copied().collect();
    ids.sort();
    for id in ids {
        println!("{id:>6}  {:?}", table[&id]);
    }
    Ok(())
}

/// The Free Pack's menus (`menus/` in the pack): each dialog's layout from
/// the player's `UI/*.DAT`, kept to where things sit and what they do
/// (control names, places, picture and sound names, text ids), with the
/// game's own wording taken out and ours put in from `text` (lines of
/// `ui.<id><TAB>text` for the menus' strings, `lang.<id><TAB>text` for the
/// game's, and `tip.<DIALOG>.<Control><TAB>tooltip`; `#` comments). Writes
/// `<DIALOG>.dtm` for each, `strings.txt` and `lang.txt`.
fn free_menus(ui: &Path, text: &Path, out: &Path) -> BoxResult {
    let words = std::fs::read_to_string(text)?;
    let mut tips = std::collections::HashMap::new();
    let mut ids = std::collections::HashMap::new();
    let mut strings = String::new();
    let mut lang = String::new();
    for line in words.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let Some((key, value)) = line.split_once('\t') else { continue };
        if let Some(id) = key.strip_prefix("ui.") {
            strings.push_str(&format!("{id}\t{}\n", value.trim()));
            ids.insert(id.trim().to_string(), value.trim().to_string());
        } else if let Some(id) = key.strip_prefix("lang.") {
            lang.push_str(&format!("{id}\t{}\n", value.trim()));
        } else if let Some(at) = key.strip_prefix("tip.") {
            tips.insert(at.to_ascii_uppercase(), value.trim().to_string());
        }
    }
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("strings.txt"), strings)?;
    std::fs::write(out.join("lang.txt"), lang)?;
    // The Pro Circuit's money, series and rival names, beside the text.
    if let Some(dir) = text.parent() {
        for name in ["pc-tables.txt", "pc-schedule.txt", "pc-names.txt"] {
            if let Ok(b) = std::fs::read(dir.join(name)) {
                std::fs::write(out.join(name), b)?;
            }
        }
    }
    let mut dats: Vec<_> = std::fs::read_dir(ui)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dat")))
        .collect();
    dats.sort();
    let (mut written, mut missing) = (0, Vec::new());
    for path in dats {
        let stem = path.file_stem().unwrap().to_string_lossy().to_ascii_uppercase();
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(ar) = rs2::Archive::parse(&bytes) else { continue };
        let Some(entry) = ar.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(".dtm")) else { continue };
        let source: String = ar.data(entry).iter().map(|&b| b as char).collect();
        let mut dtm = String::new();
        for line in source.lines() {
            let key = line.split_once('=').map(|(k, _)| k.trim().to_ascii_lowercase());
            if matches!(key.as_deref(), Some("tooltiptext" | "text" | "datarootdir")) {
                continue;
            }
            dtm.push_str(line.trim_end());
            dtm.push_str("\r\n");
            if let Some(name) = line.trim().strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                let had = source_has_tip(&source, name);
                // The control's tooltip string id, as the game's own text has it.
                let by_id = section_value(&source, name, "LocalToolTipId").and_then(|id| ids.get(id.trim()));
                if let Some(tip) = by_id.or_else(|| tips.get(&format!("{stem}.{}", name.trim().to_ascii_uppercase()))) {
                    dtm.push_str(&format!("ToolTipText = \"{}\"\r\n", tip.replace('"', "'")));
                } else if had {
                    missing.push(format!("{stem}.{}", name.trim()));
                }
            }
        }
        std::fs::write(out.join(format!("{stem}.dtm")), dtm)?;
        written += 1;
    }
    // The credits, scrolled when there's no credits movie.
    let credits_md = text.parent().and_then(Path::parent).map(|d| d.join("CREDITS.md"));
    let md = credits_md.and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    std::fs::write(out.join("credits.txt"), credits_text(&md))?;
    println!("{written} dialogs written to {}", out.display());
    if !missing.is_empty() {
        println!("{} tooltips with no wording of ours yet:", missing.len());
        for m in missing {
            println!("  tip.{m}");
        }
    }
    Ok(())
}

/// A key's value in the section `name` of a `.dtm`.
fn section_value(source: &str, name: &str, key: &str) -> Option<String> {
    let mut inside = false;
    for line in source.lines() {
        let line = line.trim();
        if let Some(n) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            inside = n.trim() == name.trim();
        } else if inside {
            if let Some((k, v)) = line.split_once('=').filter(|(k, _)| k.trim().eq_ignore_ascii_case(key)) {
                let _ = k;
                return Some(v.split('\'').next().unwrap_or("").trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

/// Whether the section `name` in a `.dtm` has a tooltip.
fn source_has_tip(source: &str, name: &str) -> bool {
    let mut inside = false;
    for line in source.lines() {
        let line = line.trim();
        if let Some(n) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            inside = n.trim() == name.trim();
        } else if inside && line.split_once('=').is_some_and(|(k, v)| k.trim().eq_ignore_ascii_case("ToolTipText") && !v.trim().trim_matches('"').is_empty()) {
            return true;
        }
    }
    false
}

/// The credits to scroll: OpenMCM2's, then each Free Pack source with the
/// people whose work it is (the last column of `CREDITS.md`'s tables).
fn credits_text(md: &str) -> String {
    let mut out = String::from(
        "# OpenMCM2\n\nOpen source motocross, free for everyone\n\n\
         # Made by\nMichael and the OpenMCM2 contributors\n\n\
         # Thanks to\nDaniel (dwolp/MCM2) for many fixes and extras\n\
         MDX Racing and every player who makes tracks\nEveryone who plays, reports bugs and donates\n\n\
         # Free Pack\nArt and sound made for OpenMCM2, and from these people:\n\n",
    );
    let mut people_col = None;
    let mut names: Vec<String> = Vec::new();
    let flush = |names: &mut Vec<String>, out: &mut String| {
        let mut seen = std::collections::BTreeSet::new();
        for n in names.drain(..) {
            for one in n.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                if seen.insert(one.to_string()) {
                    out.push_str(one);
                    out.push('\n');
                }
            }
        }
        out.push('\n');
    };
    for line in md.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            if !names.is_empty() {
                flush(&mut names, &mut out);
            } else if out.ends_with(")\n") {
                out.push('\n');
            }
            out.push_str(&format!("# {}\n", h.trim()));
            people_col = None;
        } else if line.starts_with('|') {
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            if cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ' ' || ch == ':')) {
                continue;
            }
            match people_col {
                None => people_col = cells.iter().position(|c| c.to_ascii_lowercase().starts_with("author")).or(Some(usize::MAX)),
                Some(i) if i != usize::MAX => {
                    if let Some(c) = cells.get(i) {
                        names.push(c.to_string());
                    }
                }
                _ => {}
            }
        }
    }
    if !names.is_empty() {
        flush(&mut names, &mut out);
    }
    out
}
