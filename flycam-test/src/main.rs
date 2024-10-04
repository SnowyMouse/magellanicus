#![allow(dead_code)]

use magellanicus::renderer::{AddBSPParameter, AddBSPParameterLightmapMaterial, AddBSPParameterLightmapSet, AddBitmapBitmapParameter, AddBitmapParameter, AddBitmapSequenceParameter, AddShaderBasicShaderData, AddShaderData, AddShaderParameter, AddSkyParameter, BSP3DNode, BSP3DNodeChild, BSP3DPlane, BSPCluster, BSPData, BSPLeaf, BSPPortal, BSPSubcluster, BitmapFormat, BitmapSprite, BitmapType, Renderer, RendererParameters, Resolution, SetDefaultBitmaps, ShaderType};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalSize, Size};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use clap::Parser;
use magellanicus::vertex::{LightmapVertex, ModelTriangle, ModelVertex};
use ringhopper::definitions::{Bitmap, BitmapDataFormat, BitmapDataType, Globals, Scenario, ScenarioStructureBSP, ShaderEnvironment, ShaderModel, ShaderTransparentChicago, ShaderTransparentChicagoExtended, ShaderTransparentGeneric, ShaderTransparentGlass, ShaderTransparentMeter, Sky, UnicodeStringList};
use ringhopper::primitives::dynamic::DynamicTagDataArray;
use ringhopper::primitives::engine::Engine;
use ringhopper::primitives::primitive::{TagGroup, TagPath};
use ringhopper::primitives::tag::{ParseStrictness, PrimaryTagStructDyn};
use ringhopper::tag::bitmap::MipmapTextureIterator;
use ringhopper::tag::dependency::recursively_get_dependencies_for_map;
use ringhopper::tag::scenario_structure_bsp::get_uncompressed_vertices_for_bsp_material;
use ringhopper::tag::tree::{CachingTagTree, CachingTagTreeWriteStrategy, TagTree, VirtualTagsDirectory};

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

    /// Number of viewports to use.
    ///
    /// Must be between 1 and 4.
    #[arg(long = "viewports", short = 'v', default_value = "1")]
    pub viewports: usize
}

struct ScenarioData {
    tags: HashMap<TagPath, Box<dyn PrimaryTagStructDyn>>,
    scenario_path: TagPath,
    scenario_tag: Scenario,
    engine: &'static Engine,
}

fn main() -> Result<(), String> {
    let Arguments { tags, scenario, engine, mut viewports } = Arguments::parse();

    if !(1..=4).contains(&viewports) {
        eprintln!("--viewports ({viewports}) must be between 1-4; clamping");
        viewports = viewports.clamp(1, 4);
    }

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

    let scenario_tag = dependencies
        .get(&scenario_path)
        .unwrap()
        .get_ref::<Scenario>()
        .expect("scenario wasn't scenario???")
        .to_owned();

    let scenario_data = ScenarioData {
        tags: dependencies,
        scenario_path,
        scenario_tag,
        engine,
    };

    let event_loop = EventLoop::new().unwrap();
    let mut handler = FlycamTestHandler {
        renderer: None,
        window: None,
        scenario_data,
        viewports,
        camera_speed_multiplier: 1.0,
        camera_velocity: [0.0, 0.0, 0.0],
        pause_rendering_flag: Arc::new(AtomicBool::new(false)),
    };
    event_loop.run_app(&mut handler).unwrap();
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
    window: Option<Arc<Window>>,
    scenario_data: ScenarioData,
    viewports: usize,
    pause_rendering_flag: Arc<AtomicBool>,

    camera_velocity: [f64; 3],
    camera_speed_multiplier: f64,
}

impl ApplicationHandler for FlycamTestHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut attributes = Window::default_attributes();
        attributes.inner_size = Some(Size::Physical(PhysicalSize::new(1280, 960)));
        attributes.min_inner_size = Some(Size::Physical(PhysicalSize::new(64, 64)));
        attributes.title = format!("Magellanicus - {path}", path = self.scenario_data.scenario_path);

        let window = Arc::new(event_loop.create_window(attributes).unwrap());
        self.window = Some(window.clone());

        let PhysicalSize { width, height } = window.inner_size();
        let renderer = Renderer::new(RendererParameters {
            resolution: Resolution { width, height },
            number_of_viewports: self.viewports,
            vsync: false
        }, window.clone());

        match renderer {
            Ok(r) => self.renderer = Some(Arc::new(Mutex::new(r))),
            Err(e) => {
                eprintln!("Failed to initialize renderer: {e}");
                return event_loop.exit();
            }
        }

        if let Err(e) = self.initialize_and_start() {
            eprintln!("{e}");
            return event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            },
            WindowEvent::Resized(new_size) => {
                let mut lock = self.lock_renderer();
                lock.renderer.rebuild_swapchain(RendererParameters {
                    number_of_viewports: 1,
                    vsync: false,
                    resolution: Resolution { width: new_size.width, height: new_size.height }
                }).unwrap();
            },
            _ => ()
        }
    }
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

    fn initialize_and_start(&mut self) -> Result<(), String> {
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
                fov: 70.0f32.to_radians(),
                position: [location.position.x as f32, location.position.y as f32, location.position.z as f32 + 0.7],
                rotation: {
                    let x = location.facing.angle.cos();
                    let y = location.facing.angle.sin();
                    [x, y, 0.0]
                },
            });
        }

        println!("--------------------------------------------------------------------------------");
        println!("  Loaded scenario {}...", self.scenario_data.scenario_path);
        println!("  Engine: {}", self.scenario_data.engine.display_name);
        println!("  Type: {}", self.scenario_data.scenario_tag._type);
        println!("--------------------------------------------------------------------------------");

        let render_ref = Arc::downgrade(self.renderer.as_ref().unwrap());
        let pause_rendering_ref = self.pause_rendering_flag.clone();
        std::thread::spawn(move || {
            run_renderer_thread(render_ref, pause_rendering_ref);
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

        if let Some(default_bitmaps) = self
            .scenario_data
            .tags
            .get(&TagPath::from_path("globals\\globals.globals").unwrap())
            .and_then(|g| g.get_ref::<Globals>())
            .and_then(|g| g.rasterizer_data.items.get(0))
            .and_then(|g| {
                let default_2d = g.default_2d.path().map(|p| p.to_string())?;
                let default_3d = g.default_3d.path().map(|p| p.to_string())?;
                let default_cubemap = g.default_cube_map.path().map(|p| p.to_string())?;
                Some(SetDefaultBitmaps {
                    default_2d,
                    default_3d,
                    default_cubemap
                })
            }) {
            renderer.set_default_bitmaps(default_bitmaps).map_err(|e| format!("Failed to set default bitmaps: {e}"))?
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
                    data: AddShaderData::BasicShader(AddShaderBasicShaderData {
                        bitmap: tag.diffuse.base_map.path().map(|b| b.to_string()),
                        shader_type: ShaderType::Environment,
                        alpha_tested: tag.properties.flags.alpha_tested
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

fn run_renderer_thread(renderer: Weak<Mutex<Renderer>>, pause_rendering: Arc<AtomicBool>) {
    let mut time_start = Instant::now();
    let mut frames_rendered = 0u64;
    while let Some(renderer) = renderer.upgrade() {
        if pause_rendering.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }

        let mut renderer = renderer.lock().unwrap();
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
        let time_taken = Instant::now() - time_start;
        if time_taken.as_secs() >= 1 {
            let frames_per_second = (frames_rendered as f64) / (time_taken.as_micros() as f64 / 1000000.0);
            println!("FPS: {frames_per_second}");
            frames_rendered = 0;
            time_start = Instant::now();
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
