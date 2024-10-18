#![allow(dead_code)]

use magellanicus::renderer::{AddBSPParameter, AddBSPParameterLightmapMaterial, AddBSPParameterLightmapSet, AddBitmapBitmapParameter, AddBitmapParameter, AddBitmapSequenceParameter, AddShaderBasicShaderData, AddShaderData, AddShaderEnvironmentShaderData, AddShaderParameter, AddSkyParameter, BSP3DNode, BSP3DNodeChild, BSP3DPlane, BSPCluster, BSPData, BSPLeaf, BSPPortal, BSPSubcluster, BitmapFormat, BitmapSprite, BitmapType, Renderer, RendererParameters, Resolution, ShaderType, MSAA};
use std::collections::HashMap;
use std::mem::transmute;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Instant;

use clap::Parser;
use glam::Vec3;
use magellanicus::vertex::{LightmapVertex, ModelTriangle, ModelVertex};
use ringhopper::definitions::{Bitmap, BitmapDataFormat, BitmapDataType, Scenario, ScenarioStructureBSP, ShaderEnvironment, ShaderModel, ShaderTransparentChicago, ShaderTransparentChicagoExtended, ShaderTransparentGeneric, ShaderTransparentGlass, ShaderTransparentMeter, Sky, UnicodeStringList};
use ringhopper::primitives::dynamic::DynamicTagDataArray;
use ringhopper::primitives::engine::Engine;
use ringhopper::primitives::primitive::{TagGroup, TagPath};
use ringhopper::primitives::tag::{ParseStrictness, PrimaryTagStructDyn};
use ringhopper::tag::bitmap::MipmapTextureIterator;
use ringhopper::tag::dependency::recursively_get_dependencies_for_map;
use ringhopper::tag::scenario_structure_bsp::get_uncompressed_vertices_for_bsp_material;
use ringhopper::tag::tree::{CachingTagTree, CachingTagTreeWriteStrategy, TagTree, VirtualTagsDirectory};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;

#[derive(Parser)]
struct Arguments {
    /// Tags directory(s) to use, or a single cache file.
    ///
    /// For directories, you can use --tags multiple times to specify multiple directories in order of precedent.
    #[arg(long = "tags", short = 't', default_value = "tags")]
    pub tags: Vec<String>,

    /// Path to the scenario to use relative to the tags directory(s).
    ///
    /// Ignored/not needed when loading cache files, as this is derived from the map.
    pub scenario: Option<String>,

    /// Engine to use.
    ///
    /// Ignored/not needed when loading cache files, as this is derived from the map.
    pub engine: Option<String>,

    /// Sensitivity of the mouse.
    ///
    /// Must be between 1 and 4.
    #[arg(long = "mouse-sensitivity", short = 'm', default_value = "0.0015")]
    pub mouse_sensitivity: f32,

    /// Number of viewports to use.
    ///
    /// Must be between 1 and 4.
    #[arg(long = "viewports", short = 'v', default_value = "1")]
    pub viewports: usize,

    /// MSAA setting to use.
    ///
    /// Note that your GPU may not support all options. If so, you will get an error.
    ///
    /// Must be 1, 2, 4, 8, or 16. (1 = no MSAA).
    #[arg(long = "msaa", short = 'M', default_value = "1")]
    pub msaa: u32,

    /// Enable vSync.
    ///
    /// Prevents tearing by synchronizing presenting to vertical sync. This can improve framerate
    /// stability, but it will limit performance and increase latency.
    #[arg(long = "vsync", short = 'V')]
    pub vsync: bool,

    /// Anisotropic filtering setting to use.
    ///
    /// Most GPUs support up to 16x anisotropic filtering. This setting generally improves quality
    /// without significantly affecting performance, especially on discrete GPUs.
    #[arg(long = "af", short = 'A')]
    pub anisotropic_filtering: Option<f32>,

    /// Set the resolution of the renderer.
    ///
    /// * On fullscreen mode, the default will be your monitor's resolution.
    /// * On windowed mode, the default will be your mom.
    #[arg(long = "resolution", short = 'R')]
    pub resolution: Option<String>,

    /// Use exclusive fullscreen mode.
    #[arg(long = "fullscreen", short = 'F')]
    pub fullscreen: bool

}

struct ScenarioData {
    tags: HashMap<TagPath, Box<dyn PrimaryTagStructDyn>>,
    scenario_path: TagPath,
    scenario_tag: Scenario,
    engine: &'static Engine,
}

fn main() -> Result<(), String> {
    let Arguments {
        anisotropic_filtering,
        tags,
        scenario,
        engine,
        mut viewports,
        mouse_sensitivity,
        msaa,
        vsync,
        resolution,
        fullscreen
    } = Arguments::parse();

    let sdl = sdl2::init()?;
    let mut events = sdl.event_pump()?;
    let video = sdl.video()?;
    let mouse = sdl.mouse();

    let resolution = match resolution {
        Some(resolution) => parse_resolution(resolution)?,
        None => {
            let res = video.current_display_mode(0)
                .map_err(|e| format!("Can't determine resolution: {e:?}"))?;
            if fullscreen {
                Resolution { width: res.w as u32, height: res.h as u32 }
            }
            else {
                Resolution {
                    width: ((res.w as u32) * 3 / 4).clamp(4, 1280),
                    height: ((res.h as u32) * 3 / 4).clamp(3, 960)
                }
            }
        }
    };

    if !(1..=4).contains(&viewports) {
        eprintln!("--viewports ({viewports}) must be between 1-4; clamping");
        viewports = viewports.clamp(1, 4);
    }

    let msaa = match msaa {
        1 => MSAA::NoMSAA,
        2 => MSAA::MSAA2x,
        4 => MSAA::MSAA4x,
        8 => MSAA::MSAA8x,
        16 => MSAA::MSAA16x,
        32 => MSAA::MSAA32x,
        64 => MSAA::MSAA64x,
        _ => {
            eprintln!("MSAA must be 1, 2, 4, 8, 16, 32, or 64. Disabling MSAA...");
            MSAA::NoMSAA
        }
    };

    let anisotropic_filtering = match anisotropic_filtering {
        Some(1.0..) | None => anisotropic_filtering,
        Some(n) => {
            eprintln!("Anisotropic filtering ({n}) must be 1 or higher. Disabling AF...");
            None
        }
    };

    let first_tags_dir: &Path = tags.get(0).unwrap().as_ref();

    let (scenario_path, engine, dependencies) = if tags.len() == 1 && first_tags_dir.is_file() {
        if engine.is_some() {
            eprintln!("--engine is ignored when loading cache files");
        }
        if scenario.is_some() {
            eprintln!("scenario path is ignored when loading cache files");
        }
        load_tags_from_cache(first_tags_dir)?
    }
    else {
        let Some(scenario) = scenario else {
            eprintln!("No tag path specified when --tags does not point to a cache file.");
            return Err("no tag path specified".to_owned())
        };
        let scenario_path = TagPath::from_path(&scenario)
            .map_err(|e| format!("Invalid tag path {scenario}: {e}"))?;

        let (engine, dependencies) = load_tags_from_dir(&tags, &scenario_path, engine)?;
        (scenario_path, engine, dependencies)
    };
    let window_title = format!("Magellanicus Flycam Test {scenario_path}");

    let scenario_tag = dependencies
        .get(&scenario_path)
        .unwrap()
        .get_ref::<Scenario>()
        .expect("scenario wasn't scenario???")
        .to_owned();

    let current_bsp_count = scenario_tag.structure_bsps.items.len();
    if current_bsp_count == 0 {
        return Err("No BSPs in the scenario.".to_owned());
    }

    let scenario_data = ScenarioData {
        tags: dependencies,
        scenario_path,
        scenario_tag,
        engine,
    };

    let mut window_builder = video.window(&window_title, resolution.width, resolution.height);

    window_builder
        .vulkan()
        .metal_view()
        .position_centered();

    if fullscreen {
        window_builder.fullscreen();
    }

    let mut window = window_builder
        .build()
        .unwrap();

    let renderer =
        unsafe {
            Renderer::new(&window, RendererParameters {
                resolution,
                number_of_viewports: viewports,
                vsync,
                anisotropic_filtering,
                msaa
            })
        }.unwrap();

    let mut handler = FlycamTestHandler {
        renderer: Some(Arc::new(Mutex::new(renderer))),
        scenario_data,
        viewports,
        camera_velocity: Arc::new([
            [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)],
            [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)],
            [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)],
            [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)],
        ]),
        pause_rendering_flag: Arc::new(AtomicBool::new(false)),
    };

    let (fps_send, fps_receive) = channel::<f64>();
    let (camera_send, camera_receive) = channel::<(f32, f32, usize)>();
    handler.initialize_and_start(camera_receive, fps_send)?;

    window.show();
    mouse.capture(true);
    mouse.set_relative_mouse_mode(true);

    let mut w = false;
    let mut a = false;
    let mut s = false;
    let mut d = false;
    let mut v = false;
    let mut space = false;
    let mut ctrl = false;
    let mut shift = false;
    let mut viewport_mod = 0;
    let mut current_bsp_index = 0usize;

    fn make_thing(w: bool, a: bool, s: bool, d: bool, ctrl: bool, space: bool) -> [f32; 3] {
        let mut forward = 1.0 * (w as u32 as f32) - 1.0 * (s as u32 as f32);
        let mut side = 1.0 * (a as u32 as f32) - 1.0 * (d as u32 as f32);
        let up = 1.0 * (space as u32 as f32) - 1.0 * (ctrl as u32 as f32);

        if forward != 0.0 && side != 0.0 {
            forward /= 2.0f32.sqrt();
            side /= 2.0f32.sqrt();
        }

        [forward, side, up]
    }

    let shift_speedup = 4.0;

    loop {
        if let Ok(frames_per_second) = fps_receive.try_recv() {
            println!("FPS: {frames_per_second}");
        }
        let Some(event) = events.wait_event_timeout(1000) else {
            continue;
        };
        match event {
            Event::Quit { .. } => {
                println!("EXITING!");
                break;
            }
            Event::MouseMotion { xrel, yrel, .. } => {
                let _ = camera_send.send((xrel as f32 * mouse_sensitivity, yrel as f32 * mouse_sensitivity, viewport_mod));
            }
            Event::KeyDown { keycode, repeat, .. } => {
                if repeat == true {
                    continue
                }

                if keycode == Some(Keycode::Escape) {
                    break;
                }

                if keycode == Some(Keycode::Tab) {
                    viewport_mod = (viewport_mod + 1) % viewports;
                    continue;
                }

                if keycode == Some(Keycode::PageUp) || keycode == Some(Keycode::PageDown) {
                    if current_bsp_count == 1 {
                        println!("Can't switch BSPs because there is only one.");
                        continue;
                    }

                    if keycode == Some(Keycode::PageUp) {
                        current_bsp_index = (current_bsp_index + 1) % current_bsp_count;
                    }
                    else {
                        current_bsp_index = current_bsp_index.checked_sub(1).unwrap_or(current_bsp_count - 1);
                    }

                    let path = handler
                        .scenario_data
                        .scenario_tag
                        .structure_bsps
                        .items[current_bsp_index]
                        .structure_bsp
                        .path()
                        .map(|t| t.to_string());

                    let path = path.as_ref().map(|p| p.as_str());
                    handler.lock_renderer().renderer.set_current_bsp(path).unwrap();

                    println!("Changing BSP to #{current_bsp_index} ({})", path.unwrap_or("<no BSP loaded>"));

                    continue;
                }

                if keycode == Some(Keycode::Q) {
                    let mut renderer = handler.lock_renderer();
                    let mut camera = renderer.renderer.get_camera_for_viewport(viewport_mod);
                    camera.lightmaps = !camera.lightmaps;
                    renderer.renderer.set_camera_for_viewport(viewport_mod, camera);
                    continue;
                }

                if keycode == Some(Keycode::F) {
                    let mut renderer = handler.lock_renderer();
                    let mut camera = renderer.renderer.get_camera_for_viewport(viewport_mod);
                    camera.fog = !camera.fog;
                    renderer.renderer.set_camera_for_viewport(viewport_mod, camera);
                    continue;
                }

                if keycode == Some(Keycode::R) {
                    let Some(current_bsp) = handler
                        .scenario_data
                        .scenario_tag
                        .structure_bsps
                        .items[current_bsp_index]
                        .structure_bsp
                        .path() else {
                        println!("Unable to respawn! No BSP loaded.");
                        continue;
                    };

                    let bsp: &ScenarioStructureBSP = handler
                        .scenario_data
                        .tags[current_bsp]
                        .get_ref()
                        .unwrap();

                    let x = (bsp.world_bounds_x.upper + bsp.world_bounds_x.lower) / 2.0;
                    let y = (bsp.world_bounds_y.upper + bsp.world_bounds_y.lower) / 2.0;
                    let z = (bsp.world_bounds_z.upper + bsp.world_bounds_z.lower) / 2.0;

                    let mut renderer = handler.lock_renderer();
                    let mut camera = renderer.renderer.get_camera_for_viewport(viewport_mod);
                    camera.position = [x as f32, y as f32, z as f32];
                    renderer.renderer.set_camera_for_viewport(viewport_mod, camera);
                    println!("Teleported to the center of the BSP.");
                    continue;
                }

                w |= keycode == Some(Keycode::W);
                a |= keycode == Some(Keycode::A);
                s |= keycode == Some(Keycode::S);
                d |= keycode == Some(Keycode::D);
                v |= keycode == Some(Keycode::V);
                ctrl |= keycode == Some(Keycode::LCtrl);
                space |= keycode == Some(Keycode::Space);
                shift |= keycode == Some(Keycode::LShift);

                let result = make_thing(w,a,s,d,ctrl,space);
                handler.camera_velocity[viewport_mod][0].swap(result[0].to_bits(), Ordering::Relaxed);
                handler.camera_velocity[viewport_mod][1].swap(result[1].to_bits(), Ordering::Relaxed);
                handler.camera_velocity[viewport_mod][2].swap(result[2].to_bits(), Ordering::Relaxed);

                if keycode == Some(Keycode::LShift) {
                    let increased = f32::from_bits(handler.camera_velocity[0][3].load(Ordering::Relaxed)) + shift_speedup;
                    println!("Camera #{viewport_mod} speed: {}x", camera_multiplier(increased));
                    handler.camera_velocity[viewport_mod][3].swap(increased.to_bits(), Ordering::Relaxed);
                }
            }
            Event::KeyUp { keycode, repeat, .. } => {
                if repeat == true {
                    continue
                }

                w &= keycode != Some(Keycode::W);
                a &= keycode != Some(Keycode::A);
                s &= keycode != Some(Keycode::S);
                d &= keycode != Some(Keycode::D);
                v &= keycode != Some(Keycode::V);
                ctrl &= keycode != Some(Keycode::LCtrl);
                space &= keycode != Some(Keycode::Space);
                shift &= keycode != Some(Keycode::LShift);

                let result = make_thing(w,a,s,d,ctrl,space);
                handler.camera_velocity[viewport_mod][0].swap(result[0].to_bits(), Ordering::Relaxed);
                handler.camera_velocity[viewport_mod][1].swap(result[1].to_bits(), Ordering::Relaxed);
                handler.camera_velocity[viewport_mod][2].swap(result[2].to_bits(), Ordering::Relaxed);

                if keycode == Some(Keycode::LShift) {
                    let reduced = f32::from_bits(handler.camera_velocity[viewport_mod][3].load(Ordering::Relaxed)) - shift_speedup;
                    println!("Camera #{viewport_mod} speed: {}x", camera_multiplier(reduced));
                    handler.camera_velocity[viewport_mod][3].swap(reduced.to_bits(), Ordering::Relaxed);
                }
            }
            Event::MouseWheel { x, y, .. } => {
                let incrementor = if x.abs() > y.abs() {
                    x
                }
                else {
                    y
                };

                if v {
                    let mut lock = handler.lock_renderer();
                    let mut camera = lock.renderer.get_camera_for_viewport(viewport_mod);

                    let new_fov_deg = (camera.fov.to_degrees() + incrementor as f32 * 1.0)
                        .round()
                        .clamp(1.0, 179.0);
                    camera.fov = new_fov_deg.to_radians();
                    println!("Setting camera #{viewport_mod}'s vertical FoV to {new_fov_deg:.04} ({}%) degrees", new_fov_deg / 56.0 * 100.0);

                    lock.renderer.set_camera_for_viewport(viewport_mod, camera);
                    continue
                }

                let mut multiplier = f32::from_bits(handler.camera_velocity[viewport_mod][3].load(Ordering::Relaxed));

                multiplier += (incrementor as f32) * 0.25;

                let mut min = -20.0;
                let mut max = 24.0;

                if shift {
                    min += shift_speedup;
                    max += shift_speedup;
                }

                multiplier = multiplier.clamp(min, max);

                println!("Camera #{viewport_mod} speed: {}x", camera_multiplier(multiplier));

                handler.camera_velocity[viewport_mod][3].swap(multiplier.to_bits(), Ordering::Relaxed);
            }
            _ => {

                // println!("{n:?}")
            }
        }
    }


    Ok(())
}

fn load_tags_from_dir(tags: &Vec<String>, scenario_path: &TagPath, engine: Option<String>) -> Result<(&'static Engine, HashMap<TagPath, Box<dyn PrimaryTagStructDyn>>), String> {
    let Some(engine) = engine else {
        eprintln!("You need to specify an engine if you are not loading a tags directory.");
        return Err("no engine provided".to_string());
    };

    let Some(engine) = ringhopper_engines::ALL_SUPPORTED_ENGINES
        .iter()
        .filter(|f| f.build_target && f.name == engine)
        .next() else {

        let mut valid_engines = String::new();
        for i in ringhopper_engines::ALL_SUPPORTED_ENGINES.iter().filter(|f| f.build_target) {
            valid_engines += &format!("\n - {}", i.name);
        }

        eprintln!("Invalid engine `{engine}`. Valid engines are: {valid_engines}");
        return Err("invalid engine provided".to_string());
    };

    let directories = VirtualTagsDirectory::new(&tags, None)
        .map_err(|e| format!("Error reading tags directory {tags:?}: {e}"))
        .map(|t| CachingTagTree::new(t, CachingTagTreeWriteStrategy::Instant))?;

    let mut dependencies: HashMap<TagPath, Box<dyn PrimaryTagStructDyn>> = HashMap::new();

    let dependencies_tags = recursively_get_dependencies_for_map(scenario_path, &directories, engine)
        .map_err(|e| format!("Failed to read all tags for {scenario_path}: {e}"))?
        .into_iter();

    for i in dependencies_tags {
        let tag = directories.open_tag_shared(&i)
            .map_err(|e| format!("Failed to read {i}: {e}"))?;
        let mut tag = tag
            .lock()
            .unwrap();
        let tag = &mut *tag;
        tag.set_defaults();
        let mut replacement: Box<dyn PrimaryTagStructDyn> = Box::new(UnicodeStringList::default());
        std::mem::swap(tag, &mut replacement);
        dependencies.insert(i, replacement);
    }

    Ok((engine, dependencies))
}

fn load_tags_from_cache(cache: &Path) -> Result<(TagPath, &'static Engine, HashMap<TagPath, Box<dyn PrimaryTagStructDyn>>), String> {
    let map = ringhopper::map::load_map_from_filesystem(cache, ParseStrictness::Relaxed)
        .map_err(|e| format!("Failed to read {}: {e}", e.to_string()))?;

    let mut dependencies: HashMap<TagPath, Box<dyn PrimaryTagStructDyn>> = HashMap::new();

    for i in map.get_all_tags() {
        let tag = map.open_tag_copy(&i).map_err(|e| format!("Failed to read {i}: {e}"))?;
        dependencies.insert(i, tag);
    }

    Ok((map.get_scenario_tag().tag_path.clone(), map.get_engine(), dependencies))
}

pub struct FlycamTestHandler {
    renderer: Option<Arc<Mutex<Renderer>>>,
    scenario_data: ScenarioData,
    viewports: usize,
    pause_rendering_flag: Arc<AtomicBool>,

    camera_velocity: Arc<[[AtomicU32; 4]; 4]>,
}

impl FlycamTestHandler {
    fn lock_renderer(&self) -> PriorityLock {
        while !self.pause_rendering_flag.swap(true, Ordering::Relaxed) {
            continue;
        }
        let locked_renderer = self.renderer.as_ref().unwrap();
        let renderer = locked_renderer.lock().unwrap();
        let pause_rendering_flag = self.pause_rendering_flag.clone();
        PriorityLock {
            renderer,
            pause_rendering_flag
        }
    }

    fn initialize_and_start(&mut self, camera_rotation_channel: Receiver<(f32, f32, usize)>, fps_channel: Sender<f64>) -> Result<(), String> {
        if let Err(e) = self.load_bitmaps() {
            return Err(format!("ERROR LOADING BITMAPS: {e}"))
        }

        if let Err(e) = self.load_shaders() {
            return Err(format!("ERROR LOADING shaders: {e}"))
        }

        if let Err(e) = self.load_skies() {
            return Err(format!("ERROR LOADING skies: {e}"))
        }

        if let Err(e) = self.load_bsps() {
            return Err(format!("ERROR: {e}"))
        }

        if let Some(n) = self.scenario_data.scenario_tag.structure_bsps.items.first().and_then(|b| b.structure_bsp.path()) {
            if let Err(e) = self.renderer.as_mut().unwrap().lock().unwrap().set_current_bsp(Some(&n.to_string())) {
                return Err(format!("ERROR: {e}"))
            }
        }

        let mut renderer = self.renderer.as_ref().unwrap().lock().unwrap();
        let renderer = &mut *renderer;

        for (vi, location) in self.scenario_data
            .scenario_tag
            .player_starting_locations
            .items
            .iter()
            .enumerate()
            .take(renderer.get_viewport_count()) {
            renderer.set_camera_for_viewport(vi, magellanicus::renderer::Camera {
                position: [location.position.x as f32, location.position.y as f32, location.position.z as f32 + 0.7],
                rotation: {
                    let x = location.facing.angle.cos();
                    let y = location.facing.angle.sin();
                    [x, y, 0.0]
                },
                ..Default::default()
            });
        }

        println!("--------------------------------------------------------------------------------");
        println!("  Loaded scenario {}...", self.scenario_data.scenario_path);
        println!("  Engine: {}", self.scenario_data.engine.display_name);
        println!("  Type: {}", self.scenario_data.scenario_tag._type);
        println!("--------------------------------------------------------------------------------");

        let render_ref = Arc::downgrade(self.renderer.as_ref().unwrap());
        let pause_rendering_ref = self.pause_rendering_flag.clone();
        let velocity = self.camera_velocity.clone();
        std::thread::spawn(move || {
            run_renderer_thread(render_ref, pause_rendering_ref, velocity, camera_rotation_channel, fps_channel);
        });

        Ok(())
    }

    fn load_bitmaps(&mut self) -> Result<(), String> {
        let mut renderer = self.renderer.as_mut().unwrap().lock().unwrap();
        let renderer = &mut *renderer;
        let all_bitmaps = self.scenario_data
            .tags
            .iter()
            .filter(|f| f.0.group() == TagGroup::Bitmap)
            .map(|f| (f.0, f.1.get_ref::<Bitmap>().unwrap()));
        
        for (path, bitmap) in all_bitmaps {
            Self::load_bitmap(renderer, &path, bitmap).map_err(|e| format!("Failed to load bitmap {path}: {e}"))?;
        }

        Ok(())
    }

    fn load_bitmap(renderer: &mut Renderer, path: &&TagPath, bitmap: &Bitmap) -> Result<(), String> {
        let parameter = AddBitmapParameter {
            bitmaps: {
                let mut bitmaps = Vec::with_capacity(bitmap.bitmap_data.items.len());
                for (bitmap_index, b) in bitmap.bitmap_data.items.iter().enumerate() {
                    let format = match b.format {
                        BitmapDataFormat::A8 => BitmapFormat::A8,
                        BitmapDataFormat::Y8 => BitmapFormat::Y8,
                        BitmapDataFormat::AY8 => BitmapFormat::AY8,
                        BitmapDataFormat::A8Y8 => BitmapFormat::A8Y8,
                        BitmapDataFormat::R5G6B5 => BitmapFormat::R5G6B5,
                        BitmapDataFormat::A1R5G5B5 => BitmapFormat::A1R5G5B5,
                        BitmapDataFormat::A4R4G4B4 => BitmapFormat::A4R4G4B4,
                        BitmapDataFormat::X8R8G8B8 => BitmapFormat::X8R8G8B8,
                        BitmapDataFormat::A8R8G8B8 => BitmapFormat::A8R8G8B8,
                        BitmapDataFormat::DXT1 => BitmapFormat::DXT1,
                        BitmapDataFormat::DXT3 => BitmapFormat::DXT3,
                        BitmapDataFormat::DXT5 => BitmapFormat::DXT5,
                        BitmapDataFormat::P8 => BitmapFormat::P8,
                        BitmapDataFormat::BC7 => BitmapFormat::BC7,
                    };
                    let parameter = AddBitmapBitmapParameter {
                        format,
                        bitmap_type: match b._type {
                            BitmapDataType::CubeMap => BitmapType::Cubemap,
                            BitmapDataType::_3dTexture => BitmapType::Dim3D { depth: b.depth as u32 },
                            _ => BitmapType::Dim2D
                        },
                        resolution: Resolution { width: b.width as u32, height: b.height as u32 },
                        mipmap_count: b.mipmap_count as u32,
                        data: {
                            let length = MipmapTextureIterator::new_from_bitmap_data(b)
                                .map_err(|e| format!("Error with reading bitmap data #{bitmap_index} from {path}: {e:?}"))?
                                .map(|b| b.block_count)
                                .reduce(|a, b| a + b)
                                .unwrap() * format.block_byte_size();
                            let start = b.pixel_data_offset as usize;
                            let data: &[u8] = start.checked_add(length)
                                .and_then(|end| bitmap.processed_pixel_data.bytes.get(start..end))
                                .ok_or_else(|| format!("Can't read {length} bytes from {start} in a buffer of {} bytes for bitmap data #{bitmap_index} in {path}", bitmap.processed_pixel_data.bytes.len()))?;
                            data.to_vec()
                        }
                    };
                    bitmaps.push(parameter);
                }
                bitmaps
            },
            sequences: {
                let mut sequences = Vec::with_capacity(bitmap.bitmap_group_sequence.items.len());
                for (sequence_index, s) in bitmap.bitmap_group_sequence.items.iter().enumerate() {
                    let result = if bitmap._type == ringhopper::definitions::BitmapType::Sprites {
                        AddBitmapSequenceParameter::Sprites {
                            sprites: {
                                let mut sprites = Vec::with_capacity(s.sprites.items.len());
                                for (sprite_index, s) in s.sprites.items.iter().enumerate() {
                                    let sprite = BitmapSprite {
                                        bitmap: s.bitmap_index.map(|o| o as usize).ok_or_else(|| format!("Sprite {sprite_index} of sequence {sequence_index} of bitmap {path} has a null bitmap index"))?,
                                        top: s.top as f32,
                                        left: s.left as f32,
                                        bottom: s.bottom as f32,
                                        right: s.right as f32
                                    };
                                    sprites.push(sprite);
                                }
                                sprites
                            }
                        }
                    } else {
                        let mut first_bitmap_index = s.first_bitmap_index
                            .map(|o| o as usize);

                        if first_bitmap_index.is_none() {
                            if s.bitmap_count > 0 {
                                return Err(format!("Sequence {sequence_index} of bitmap {path} has a null bitmap index"))
                            }
                            else {
                                first_bitmap_index = Some(0)
                            }
                        }

                        AddBitmapSequenceParameter::Bitmap {
                            first: first_bitmap_index.unwrap(),
                            count: s.bitmap_count as usize
                        }
                    };
                    sequences.push(result);
                }
                sequences
            }
        };

        renderer.add_bitmap(&path.to_string(), parameter).map_err(|e| e.to_string())
    }

    fn load_shaders(&mut self) -> Result<(), String> {
        let mut renderer = self.renderer.as_mut().unwrap().lock().unwrap();
        let renderer = &mut *renderer;

        let all_shaders = self.scenario_data
            .tags
            .iter()
            .filter(|f| f.0.group().subgroup() == Some(TagGroup::Shader));

        for (path, tag) in all_shaders {
            Self::load_shader(renderer, &path, tag).map_err(|e| format!("Failed to load shader {path}: {e}"))?;
        }

        Ok(())
    }

    fn load_shader(renderer: &mut Renderer, path: &&TagPath, tag: &Box<dyn PrimaryTagStructDyn>) -> Result<(), String> {
        let new_shader = match tag.group() {
            TagGroup::ShaderEnvironment => {
                let tag = tag.get_ref::<ShaderEnvironment>().unwrap();
                AddShaderParameter {
                    data: AddShaderData::ShaderEnvironment(AddShaderEnvironmentShaderData {
                        alpha_tested: tag.properties.flags.alpha_tested,
                        bump_map_is_specular_mask: tag.properties.flags.bump_map_is_specular_mask,
                        base_map: tag.diffuse.base_map.path().map(|p| p.to_string()),
                        primary_detail_map: tag.diffuse.primary_detail_map.path().map(|p| p.to_string()),
                        secondary_detail_map: tag.diffuse.secondary_detail_map.path().map(|p| p.to_string()),
                        micro_detail_map: tag.diffuse.micro_detail_map.path().map(|p| p.to_string()),
                        bump_map: tag.bump.bump_map.path().map(|p| p.to_string()),
                        reflection_cube_map: tag.reflection.reflection_cube_map.path().map(|p| p.to_string()),
                        primary_detail_map_scale: tag.diffuse.primary_detail_map_scale as f32,
                        secondary_detail_map_scale: tag.diffuse.secondary_detail_map_scale as f32,
                        micro_detail_map_scale: tag.diffuse.micro_detail_map_scale as f32,
                        bump_map_scale: tag.bump.bump_map_scale as f32,
                        parallel_color: [
                            tag.specular.parallel_color.red as f32,
                            tag.specular.parallel_color.green as f32,
                            tag.specular.parallel_color.blue as f32,
                        ],
                        perpendicular_color: [
                            tag.specular.perpendicular_color.red as f32,
                            tag.specular.perpendicular_color.green as f32,
                            tag.specular.perpendicular_color.blue as f32,
                        ],
                        parallel_brightness: tag.reflection.parallel_brightness as f32,
                        perpendicular_brightness: tag.reflection.perpendicular_brightness as f32,

                        // SAFETY: 🔥🐶🔥 This is fine 🔥🐶🔥
                        shader_environment_type: unsafe { transmute(tag.properties.shader_environment_type as u32) },
                        detail_map_function: unsafe { transmute(tag.diffuse.detail_map_function as u32) },
                        micro_detail_map_function: unsafe { transmute(tag.diffuse.micro_detail_map_function as u32) },
                        reflection_type: unsafe { transmute(tag.reflection._type as u32) },
                    })
                }
            },
            TagGroup::ShaderModel => {
                let tag = tag.get_ref::<ShaderModel>().unwrap();
                AddShaderParameter {
                    data: AddShaderData::BasicShader(AddShaderBasicShaderData {
                        bitmap: tag.maps.base_map.path().map(|q| q.to_string()),
                        shader_type: ShaderType::Model,
                        alpha_tested: !tag.properties.flags.not_alpha_tested
                    })
                }
            },
            TagGroup::ShaderTransparentChicago => {
                let tag = tag.get_ref::<ShaderTransparentChicago>().unwrap();
                AddShaderParameter {
                    data: AddShaderData::BasicShader(AddShaderBasicShaderData {
                        bitmap: tag
                            .maps
                            .items
                            .get(0)
                            .and_then(|b| b.parameters.map.path())
                            .map(|b| b.to_string()),
                        shader_type: ShaderType::TransparentChicago,
                        alpha_tested: true
                    })
                }
            },
            TagGroup::ShaderTransparentChicagoExtended => {
                let tag = tag.get_ref::<ShaderTransparentChicagoExtended>().unwrap();
                AddShaderParameter {
                    data: AddShaderData::BasicShader(AddShaderBasicShaderData {
                        bitmap: tag
                            ._4_stage_maps
                            .items
                            .get(0)
                            .and_then(|b| b.parameters.map.path())
                            .map(|b| b.to_string()),
                        shader_type: ShaderType::TransparentChicago,
                        alpha_tested: true
                    })
                }
            },
            TagGroup::ShaderTransparentGeneric => {
                let tag = tag.get_ref::<ShaderTransparentGeneric>().unwrap();
                AddShaderParameter {
                    data: AddShaderData::BasicShader(AddShaderBasicShaderData {
                        bitmap: tag
                            .maps
                            .items
                            .get(0)
                            .and_then(|b| b.parameters.map.path())
                            .map(|b| b.to_string()),
                        shader_type: ShaderType::TransparentGeneric,
                        alpha_tested: true
                    })
                }
            },
            TagGroup::ShaderTransparentGlass => {
                let tag = tag.get_ref::<ShaderTransparentGlass>().unwrap();
                AddShaderParameter {
                    data: AddShaderData::BasicShader(AddShaderBasicShaderData {
                        bitmap: tag
                            .diffuse
                            .diffuse_map
                            .path()
                            .map(|b| b.to_string()),
                        shader_type: ShaderType::TransparentGlass,
                        alpha_tested: true
                    })
                }
            },
            TagGroup::ShaderTransparentMeter => {
                let tag = tag.get_ref::<ShaderTransparentMeter>().unwrap();
                AddShaderParameter {
                    data: AddShaderData::BasicShader(AddShaderBasicShaderData {
                        bitmap: tag
                            .properties
                            .map
                            .path()
                            .map(|b| b.to_string()),
                        shader_type: ShaderType::TransparentMeter,
                        alpha_tested: true
                    })
                }
            },
            TagGroup::ShaderTransparentPlasma => {
                // let tag = tag.get_ref::<ShaderTransparentPlasma>().unwrap();
                AddShaderParameter {
                    data: AddShaderData::BasicShader(AddShaderBasicShaderData {
                        bitmap: None,
                        shader_type: ShaderType::TransparentPlasma,
                        alpha_tested: true
                    })
                }
            },
            TagGroup::ShaderTransparentWater => {
                // let tag = tag.get_ref::<ShaderTransparentWater>().unwrap();
                AddShaderParameter {
                    data: AddShaderData::BasicShader(AddShaderBasicShaderData {
                        bitmap: None,
                        shader_type: ShaderType::TransparentWater,
                        alpha_tested: true
                    })
                }
            },
            n => unreachable!("{n}")
        };
        renderer.add_shader(&path.to_string(), new_shader).map_err(|e| e.to_string())
    }

    fn load_skies(&mut self) -> Result<(), String> {
        let mut renderer = self.renderer.as_mut().unwrap().lock().unwrap();
        let renderer = &mut *renderer;

        let all_skies = self.scenario_data
            .tags
            .iter()
            .filter(|f| f.0.group() == TagGroup::Sky);

        for (path, tag) in all_skies {
            Self::load_sky(renderer, path, tag.get_ref().unwrap()).map_err(|e| format!("Failed to load sky {path}: {e}"))?;
        }

        Ok(())
    }

    fn load_sky(renderer: &mut Renderer, path: &TagPath, sky: &Sky) -> Result<(), String> {
        renderer.add_sky(&path.to_string(), AddSkyParameter {
            geometry: None,
            outdoor_fog_color: [sky.outdoor_fog.color.red as f32, sky.outdoor_fog.color.green as f32, sky.outdoor_fog.color.blue as f32],
            outdoor_fog_maximum_density: sky.outdoor_fog.maximum_density as f32,
            outdoor_fog_start_distance: sky.outdoor_fog.start_distance as f32,
            outdoor_fog_opaque_distance: sky.outdoor_fog.opaque_distance as f32,
            indoor_fog_color: [sky.indoor_fog.color.red as f32, sky.indoor_fog.color.green as f32, sky.indoor_fog.color.blue as f32],
            indoor_fog_maximum_density: sky.indoor_fog.maximum_density as f32,
            indoor_fog_start_distance: sky.indoor_fog.start_distance as f32,
            indoor_fog_opaque_distance: sky.indoor_fog.opaque_distance as f32,
        }).map_err(|e| e.to_string())
    }

    fn load_bsps(&mut self) -> Result<(), String> {
        let mut renderer = self.renderer.as_mut().unwrap().lock().unwrap();
        let renderer = &mut *renderer;

        let all_bsps = self.scenario_data
            .tags
            .iter()
            .filter(|f| f.0.group() == TagGroup::ScenarioStructureBSP)
            .map(|f| (f.0, f.1.get_ref::<ScenarioStructureBSP>().unwrap()));

        for (path, bsp) in all_bsps {
            let mut add_bsp = AddBSPParameter {
                lightmap_bitmap: bsp.lightmaps_bitmap.path().map(|p| p.to_native_path()),
                lightmap_sets: Vec::with_capacity(bsp.lightmaps.items.len()),
                bsp_data: BSPData {
                    nodes: bsp.collision_bsp.items[0].bsp3d_nodes.items.iter().map(|i| BSP3DNode {
                        front_child: BSP3DNodeChild::from_flagged_u32(i.front_child),
                        back_child: BSP3DNodeChild::from_flagged_u32(i.back_child),
                        plane: i.plane as usize
                    }).collect(),
                    planes: bsp.collision_bsp.items[0].planes.items.iter().map(|i| BSP3DPlane {
                        angle: [i.plane.vector.x as f32, i.plane.vector.y as f32, i.plane.vector.z as f32],
                        offset: i.plane.d as f32
                    }).collect(),
                    leaves: bsp.leaves.items.iter().map(|i| BSPLeaf {
                        cluster: i.cluster.unwrap() as usize
                    }).collect(),
                    clusters: bsp.clusters.items.iter().map(|i| BSPCluster {
                        sky: if let Some(sky) = i.sky {
                            self.scenario_data
                                .scenario_tag
                                .skies
                                .items.get(sky as usize)
                                .ok_or_else(|| format!("BSP {path} references sky {sky} which isn't valid on the scenario"))
                                .unwrap()
                                .sky
                                .path()
                                .map(|s| s.to_string())
                        }
                        else {
                            None
                        },
                        subclusters: i.subclusters.items.iter().map(|s| BSPSubcluster {
                            surface_indices: s.surface_indices.items.iter().map(|i| i.index as usize).collect(),
                            world_bounds_from: [s.world_bounds_x.lower as f32, s.world_bounds_y.lower as f32, s.world_bounds_z.lower as f32],
                            world_bounds_to: [s.world_bounds_x.upper as f32, s.world_bounds_y.upper as f32, s.world_bounds_z.upper as f32],
                        }).collect(),
                        cluster_portals: i.portals.items.iter().map(|s| s.portal.unwrap_or(0xFFFF) as usize).collect()
                    }).collect(),
                    portals: bsp.cluster_portals.items.iter().map(|p| BSPPortal {
                        front_cluster: p.front_cluster.unwrap_or(0xFFFF) as usize,
                        back_cluster: p.back_cluster.unwrap_or(0xFFFF) as usize,
                    }).collect()
                },
            };

            for (lightmap_index, lightmap) in bsp.lightmaps.items.iter().enumerate() {
                let mut add_lightmap = AddBSPParameterLightmapSet {
                    lightmap_index: lightmap.bitmap.map(|i| i as usize),
                    materials: Vec::with_capacity(lightmap.materials.len())
                };

                for (material_index, material) in lightmap.materials.items.iter().enumerate() {
                    let Some(shader_path) = material.shader.path() else {
                        continue
                    };

                    let surfaces: usize = material.surfaces.try_into().unwrap();
                    let surface_count: usize = material.surface_count.try_into().unwrap();

                    let surface_indices = surfaces.checked_add(surface_count)
                        .and_then(|range_end| bsp
                            .surfaces
                            .items
                            .get(surfaces..range_end)
                        );
                    let Some(surface_indices) = surface_indices else {
                        return Err(format!("Material #{material_index} of Lightmap #{lightmap_index} of BSP {path} has broken surface indices."));
                    };

                    let indices = surface_indices
                        .iter()
                        .filter_map(|s| {
                            let a = s.vertex0_index?;
                            let b = s.vertex1_index?;
                            let c = s.vertex2_index?;
                            Some(ModelTriangle { indices: [a,b,c] })
                    }).collect();

                    let (material, lightmap) = get_uncompressed_vertices_for_bsp_material(material).map_err(|e| {
                        format!("Material #{material_index} of Lightmap #{lightmap_index} of BSP {path} has broken vertices: {e:?}")
                    })?;

                    let shader_vertices = material
                        .map(|f| ModelVertex {
                            position: [f.position.x as f32, f.position.y as f32, f.position.z as f32],
                            normal: [f.normal.x as f32, f.normal.y as f32, f.normal.z as f32],
                            binormal: [f.binormal.x as f32, f.binormal.y as f32, f.binormal.z as f32],
                            tangent: [f.tangent.x as f32, f.tangent.y as f32, f.tangent.z as f32],
                            texture_coords: [f.texture_coords.x as f32, f.texture_coords.y as f32]
                        })
                        .collect();

                    let lightmap: Vec<LightmapVertex> = lightmap
                        .map(|f| LightmapVertex {
                            lightmap_texture_coords: [f.texture_coords.x as f32, f.texture_coords.y as f32]
                        })
                        .collect();

                    add_lightmap.materials.push(AddBSPParameterLightmapMaterial {
                        shader_vertices,
                        lightmap_vertices: (!lightmap.is_empty()).then_some(lightmap),
                        surfaces: indices,
                        shader: shader_path.to_native_path()
                    });
                }
                add_bsp.lightmap_sets.push(add_lightmap);
            }

            renderer.add_bsp(&path.to_native_path(), add_bsp).map_err(|e| format!("Failed to load BSP {path}: {e}"))?;
        }

        Ok(())
    }
}

fn run_renderer_thread(renderer: Weak<Mutex<Renderer>>, pause_rendering: Arc<AtomicBool>, velocity: Arc<[[AtomicU32; 4]; 4]>, camera_channel: Receiver<(f32, f32, usize)>, fps_channel: Sender<f64>) {
    let time_start = Instant::now();
    let mut last_loop = 0.0;
    let mut time_since_last_fps = Instant::now();
    let mut frames_rendered = 0u64;
    while let Some(renderer) = renderer.upgrade() {
        if pause_rendering.load(Ordering::Relaxed) {
            continue;
        }

        let mut renderer = renderer.lock().unwrap();

        let ms_since_start = (Instant::now() - time_start).as_millis() as f64 / 1000.0;
        let mut rotate_deltas = [[0.0f32; 2]; 4];

        while let Ok(n) = camera_channel.try_recv() {
            rotate_deltas[n.2][0] += n.0;
            rotate_deltas[n.2][1] += n.1;
        }

        for v in 0..renderer.get_viewport_count().min(velocity.len()) {
            let vel = &velocity[v];
            let rot = &rotate_deltas[v];

            let multiplier = camera_multiplier(f32::from_bits(vel[3].load(Ordering::Relaxed)));

            let delta = (ms_since_start - last_loop) as f32 * 2.0 * multiplier;
            let forward = f32::from_bits(vel[0].load(Ordering::Relaxed)) * delta;
            let side = f32::from_bits(vel[1].load(Ordering::Relaxed)) * delta;
            let up = f32::from_bits(vel[2].load(Ordering::Relaxed)) * delta;

            let mut camera = renderer.get_camera_for_viewport(v);
            let mut position = Vec3::from(camera.position);
            camera.rotation = rotate(camera.rotation, rot[0], rot[1]);

            let rotation = Vec3::from(camera.rotation);
            position += Vec3::new(rotation.x * forward, rotation.y * forward, rotation.z * forward);

            let q = Vec3::new(rotation.x, rotation.y, 0.0).normalize();
            position -= Vec3::new(q.y * side, -q.x * side, 0.0);
            position += Vec3::new(0.0, 0.0, up);

            camera.position = position.to_array();
            renderer.set_camera_for_viewport(v, camera);
        }

        last_loop = ms_since_start;

        let frame_result = renderer.draw_frame();
        drop(renderer);

        match frame_result {
            Ok(n) => {
                if !n {
                    continue;
                }
            },
            Err(e) => {
                eprintln!("Render fail: {e}");
                continue;
            }
        }

        frames_rendered += 1;
        let time_taken = Instant::now() - time_since_last_fps;
        if time_taken.as_secs() >= 1 {
            let frames_per_second = (frames_rendered as f64) / (time_taken.as_micros() as f64 / 1000000.0);
            let _ = fps_channel.send(frames_per_second);
            frames_rendered = 0;
            time_since_last_fps = Instant::now();
        }
    }
}

struct PriorityLock<'a> {
    renderer: MutexGuard<'a, Renderer>,
    pause_rendering_flag: Arc<AtomicBool>
}

impl Drop for PriorityLock<'_> {
    fn drop(&mut self) {
        self.pause_rendering_flag.swap(false, Ordering::Relaxed);
    }
}


fn rotate(rotation: [f32; 3], yaw_delta: f32, pitch_delta: f32) -> [f32; 3] {
    let mut yaw;
    let mut pitch;

    if rotation[0] != 0.0 || rotation[1] != 0.0 {
        let full_xyz = Vec3::from([rotation[0], rotation[1], rotation[2]]).normalize();
        let full_xy = Vec3::from([full_xyz.x, full_xyz.y, 0.0]);
        let normalized_xy = Vec3::from(full_xy).normalize();
        let x_angle = normalized_xy[0].acos();
        let y_angle = normalized_xy[1].asin();

        yaw = if y_angle < 0.0 { -x_angle } else { x_angle };
        pitch = full_xyz[2].asin();
    }
    else {
        yaw = 0.0;
        pitch = if rotation[2] < 0.0 {
            -1.5
        }
        else {
            1.5
        }
    }

    yaw -= yaw_delta;
    pitch -= pitch_delta;
    pitch = pitch.clamp(-1.5, 1.5);
    let pitch_sine = pitch.sin();
    let pitch_cosine = pitch.cos();
    [yaw.cos() * pitch_cosine, yaw.sin() * pitch_cosine, pitch_sine]
}

fn parse_resolution(resolution_string: String) -> Result<Resolution, String> {
    if resolution_string.chars().filter(|c| *c == 'x' || *c == ',').count() != 1 {
        return Err(format!("Invalid resolution {resolution_string}; bad format"));
    }
    let first_comma = resolution_string
        .find("x")
        .unwrap_or_else(|| resolution_string.find(",").unwrap());
    let (width,height) = resolution_string.split_at(first_comma);
    let Ok((width,height)) = width.parse::<u32>()
        .and_then(|w| Ok((w, height[1..].parse::<u32>()?))) else {
        return Err(format!("Invalid resolution {resolution_string}; must be numberxnumber or number,number"));
    };
    if width == 0 || height == 0 {
        return Err(format!("Invalid resolution {resolution_string}; at least one dimension is zero"));
    }
    Ok(Resolution { width, height })
}

#[inline(always)]
fn camera_multiplier(v: f32) -> f32 {
    1.25f32.powf(v)
}
