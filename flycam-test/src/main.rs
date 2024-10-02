use magellanicus::renderer::{AddBSPParameter, AddBSPParameterLightmapMaterial, AddBSPParameterLightmapSet, AddBitmapBitmapParameter, AddBitmapParameter, AddBitmapSequenceParameter, AddShaderBasicShaderData, AddShaderData, AddShaderParameter, BitmapFormat, BitmapSprite, BitmapType, Renderer, RendererParameters, Resolution, SetDefaultBitmaps, ShaderType};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalSize, Size};
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use clap::Parser;
use magellanicus::vertex::{LightmapVertex, ModelTriangle, ModelVertex};
use ringhopper::definitions::{Bitmap, BitmapDataFormat, Globals, Scenario, ScenarioStructureBSP, ShaderEnvironment, ShaderModel, ShaderTransparentChicago, ShaderTransparentChicagoExtended, ShaderTransparentGeneric, ShaderTransparentGlass, ShaderTransparentMeter, UnicodeStringList};
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
    let Arguments { tags, scenario, engine, viewports } = Arguments::parse();

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
        frame_time_counts: Vec::with_capacity(300),
        last_frame: Instant::now(),
        viewports,
        camera_speed_multiplier: 1.0,
        camera_velocity: [0.0, 0.0, 0.0]
    };
    event_loop.set_control_flow(ControlFlow::Poll);
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
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
    scenario_data: ScenarioData,
    frame_time_counts: Vec<Duration>,
    viewports: usize,

    camera_velocity: [f64; 3],
    camera_speed_multiplier: f64,
    last_frame: Instant
}

impl ApplicationHandler for FlycamTestHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut attributes = Window::default_attributes();
        attributes.inner_size = Some(Size::Physical(PhysicalSize::new(1280, 960)));
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
            Ok(r) => self.renderer = Some(r),
            Err(e) => {
                eprintln!("Failed to initialize renderer: {e}");
                event_loop.exit();
            }
        }

        if let Err(e) = self.load_bitmaps() {
            eprintln!("ERROR LOADING BITMAPS: {e}");
            return event_loop.exit();
        }

        if let Err(e) = self.load_shaders() {
            eprintln!("ERROR LOADING shaders: {e}");
            return event_loop.exit();
        }

        if let Err(e) = self.load_bsps() {
            eprintln!("ERROR: {e}");
            return event_loop.exit();
        }

        if let Some(n) = self.scenario_data.scenario_tag.structure_bsps.items.first().and_then(|b| b.structure_bsp.path()) {
            if let Err(e) = self.renderer.as_mut().unwrap().set_current_bsp(Some(&n.to_string())) {
                eprintln!("ERROR: {e}");
                return event_loop.exit();
            }
        }

        let renderer = self.renderer.as_mut().unwrap();

        for (vi, location) in self.scenario_data
            .scenario_tag
            .player_starting_locations
            .items
            .iter()
            .enumerate()
            .take(renderer.get_viewport_count()) {
            renderer.set_camera_for_viewport(vi, magellanicus::renderer::Camera {
                fov: 70.0f32.to_radians(),
                position: magellanicus::renderer::Vec3::new(location.position.x as f32, location.position.y as f32, location.position.z as f32 + 0.7),
                rotation: {
                    let x = location.facing.angle.cos();
                    let y = location.facing.angle.sin();
                    magellanicus::renderer::Vec3::new(x, y, 0.0)
                },
            });
        }

        println!("--------------------------------------------------------------------------------");
        println!("  Loaded scenario {}...", self.scenario_data.scenario_path);
        println!("  Engine: {}", self.scenario_data.engine.display_name);
        println!("  Type: {}", self.scenario_data.scenario_tag._type);
        println!("--------------------------------------------------------------------------------");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            },
            WindowEvent::KeyboardInput {
                event, ..
            } => {
                let is_pressed = event.state.is_pressed();

            }
            _ => ()
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Poll {
            if let Some(renderer) = self.renderer.as_mut() {
                let time_start = Instant::now();

                if self.camera_velocity != [0.0,0.0,0.0] {
                    let time_since_last_frame = time_start - self.last_frame;
                    let seconds = time_since_last_frame.as_micros() as f64 / 1000000.0;

                    let delta_x = self.camera_velocity[0] * seconds * self.camera_speed_multiplier;
                    let delta_y = self.camera_velocity[1] * seconds * self.camera_speed_multiplier;
                    let delta_z = self.camera_velocity[2] * seconds * self.camera_speed_multiplier;


                }


                self.last_frame = time_start;
                match renderer.draw_frame() {
                    Ok(n) => {
                        if !n {
                            let window = self.window.clone().unwrap();
                            if let Err(e) = renderer.rebuild_swapchain(
                                RendererParameters {
                                    resolution: Resolution {
                                        width: window.inner_size().width,
                                        height: window.inner_size().height,
                                    },
                                    number_of_viewports: 1,
                                    vsync: false
                                }
                            ) {
                                eprintln!("Critical error rebuilding swapchain: {e}");
                                event_loop.exit();
                                return;
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("Render fail: {e}");
                    }
                }
                let time_end = Instant::now() - time_start;
                self.frame_time_counts.push(time_end);
                if self.frame_time_counts.len() == 200 {
                    let mut time = 0u128;
                    for t in &self.frame_time_counts {
                        time = time.saturating_add(t.as_nanos());
                    }

                    let frames_per_second = (self.frame_time_counts.len() as f64) / (time as f64 / 1000000000.0);
                    println!("FPS: {frames_per_second}");

                    self.frame_time_counts.clear();
                }
                return
            }
        }
    }
}

impl FlycamTestHandler {
    fn load_bitmaps(&mut self) -> Result<(), String> {
        let renderer = self.renderer.as_mut().unwrap();
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
                        bitmap_type: match bitmap._type {
                            ringhopper::definitions::BitmapType::CubeMaps => BitmapType::Cubemap,
                            ringhopper::definitions::BitmapType::_3dTextures => BitmapType::Dim3D { depth: b.depth as u32 },
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
                        AddBitmapSequenceParameter::Bitmap {
                            first: s.first_bitmap_index.map(|o| o as usize).ok_or_else(|| format!("Sequence {sequence_index} of bitmap {path} has a null bitmap index"))?,
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
        let renderer = self.renderer.as_mut().unwrap();

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

    fn load_bsps(&mut self) -> Result<(), String> {
        let renderer = self.renderer.as_mut().unwrap();

        let all_bsps = self.scenario_data
            .tags
            .iter()
            .filter(|f| f.0.group() == TagGroup::ScenarioStructureBSP)
            .map(|f| (f.0, f.1.get_ref::<ScenarioStructureBSP>().unwrap()));

        for (path, bsp) in all_bsps {
            let mut add_bsp = AddBSPParameter {
                lightmap_bitmap: bsp.lightmaps_bitmap.path().map(|p| p.to_native_path()),
                lightmap_sets: Vec::with_capacity(bsp.lightmaps.items.len())
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
                        indices,
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
