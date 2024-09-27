use magellanicus::renderer::{AddBSPParameter, AddBSPParameterLightmapMaterial, AddBSPParameterLightmapSet, Renderer, RendererParameters, Resolution};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalSize, Size};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use clap::Parser;
use ringhopper::definitions::{Scenario, ScenarioStructureBSP, UnicodeStringList};
use ringhopper::primitives::dynamic::DynamicTagDataArray;
use ringhopper::primitives::engine::Engine;
use ringhopper::primitives::primitive::{TagGroup, TagPath};
use ringhopper::primitives::tag::{ParseStrictness, PrimaryTagStructDyn};
use ringhopper::tag::dependency::recursively_get_dependencies_for_map;
use ringhopper::tag::scenario_structure_bsp::get_uncompressed_vertices_for_bsp_material;
use ringhopper::tag::tree::{CachingTagTree, CachingTagTreeWriteStrategy, TagTree, VirtualTagsDirectory};
use magellanicus::vertex::{LightmapVertex, ModelTriangle, ModelVertex};

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
    pub engine: Option<String>
}

struct ScenarioData {
    tags: HashMap<TagPath, Box<dyn PrimaryTagStructDyn>>,
    scenario_path: TagPath,
    scenario_tag: Scenario,
    engine: &'static Engine,
}

fn main() -> Result<(), String> {
    let Arguments { tags, scenario, engine } = Arguments::parse();

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
        engine
    };

    let event_loop = EventLoop::new().unwrap();
    let mut handler = FlycamTestHandler {
        renderer: None,
        window: None,
        scenario_data
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
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
    scenario_data: ScenarioData
}

impl ApplicationHandler for FlycamTestHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut attributes = Window::default_attributes();
        attributes.inner_size = Some(Size::Physical(PhysicalSize::new(640, 480)));
        attributes.title = format!("Magellanicus - {path}", path = self.scenario_data.scenario_path);

        let window = Arc::new(event_loop.create_window(attributes).unwrap());
        self.window = Some(window.clone());

        let PhysicalSize { width, height } = window.inner_size();
        let renderer = Renderer::new(RendererParameters {
            resolution: Resolution { width, height },
            number_of_viewports: 1
        }, window.clone());

        match renderer {
            Ok(r) => self.renderer = Some(r),
            Err(e) => {
                eprintln!("Failed to initialize renderer: {e}");
                event_loop.exit();
            }
        }

        if let Err(e) = self.load_bsps() {
            eprintln!("ERROR: {e}");
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            _ => ()
        }
    }
}

impl FlycamTestHandler {
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

                    let lightmap = lightmap
                        .map(|f| LightmapVertex {
                            lightmap_texture_coords: [f.texture_coords.x as f32, f.texture_coords.y as f32]
                        })
                        .collect();

                    add_lightmap.materials.push(AddBSPParameterLightmapMaterial {
                        shader_vertices,
                        lightmap_vertices: Some(lightmap),
                        indices,
                        shader: shader_path.to_native_path()
                    });
                }
                add_bsp.lightmap_sets.push(add_lightmap);
            }

            renderer.add_bsp(&path.to_native_path(), add_bsp)?;
        }

        Ok(())
    }
}
