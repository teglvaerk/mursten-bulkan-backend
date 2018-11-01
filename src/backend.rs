use mursten::{Backend, Data, RenderChain, UpdateChain};

use nalgebra::*;

use shaders;

use std::mem;
use std::sync::Arc;

use vulkano_win::required_extensions;
use vulkano_win::VkSurfaceBuild;

use vulkano::buffer::BufferUsage;
use vulkano::buffer::CpuAccessibleBuffer;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::command_buffer::DynamicState;
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::descriptor::PipelineLayoutAbstract;
use vulkano::device::Device;
use vulkano::device::DeviceExtensions;
use vulkano::device::Queue;
use vulkano::format::Format;
use vulkano::framebuffer::Framebuffer;
use vulkano::framebuffer::RenderPass;
use vulkano::framebuffer::RenderPassAbstract;
use vulkano::framebuffer::RenderPassDesc;
use vulkano::framebuffer::Subpass;
use vulkano::image::attachment::AttachmentImage;
use vulkano::image::traits::ImageAccess;
use vulkano::image::ImageUsage;
use vulkano::image::SwapchainImage;
use vulkano::instance::Instance;
use vulkano::instance::PhysicalDevice;
use vulkano::pipeline::vertex::SingleBufferDefinition;
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::swapchain;
use vulkano::swapchain::AcquireError;
use vulkano::swapchain::PresentMode;
use vulkano::swapchain::Surface;
use vulkano::swapchain::SurfaceTransform;
use vulkano::swapchain::Swapchain;
use vulkano::swapchain::SwapchainCreationError;
use vulkano::sync::now;
use vulkano::sync::GpuFuture;

use winit::CursorState;
use winit::Event;
use winit::EventsLoop;
use winit::Window;
use winit::WindowBuilder;
use winit::WindowEvent;
use winit::KeyboardInput;


#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Uniforms {
    pub projection_view: Matrix4<f32>,

    pub light_color: Vector4<f32>,
    pub light_origin: Vector4<f32>,

    pub ambient_light_strength: f32,
    pub diffuse_light_strength: f32,
    pub specular_light_strength: f32,
}

impl Default for Uniforms {
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn default() -> Self {
        Self {
            projection_view: Orthographic3::new(-1.0, 1.0, -1.0, 1.0, -900.0, 900.0).to_homogeneous() * Matrix4::new(
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ),
            light_color: Vector4::new(1.0, 1.0, 1.0, 1.0),
            light_origin: Vector4::new(3.0, 3.0, -3.0, 1.0),
            ambient_light_strength: 0.2,
            diffuse_light_strength: 0.7,
            specular_light_strength: 0.3,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub position: [f32; 4],
    pub normal: [f32; 4],
    pub color: [f32; 4],
    pub texture: [f32; 2],
}
impl_vertex!(Vertex, position, normal, color, texture);

pub struct VulkanBackend {
    vertex_queue: Vec<Vertex>,
    event_queue: Vec<Event>,

    mouse_position: (f64, f64),

    dimensions: (u32, u32),
    constants: Uniforms,

    enable_validation_layers: bool,
    desired_validation_layer: &'static str,
}

impl VulkanBackend {
    pub fn new() -> Self {
        Self {
            vertex_queue: Vec::new(),
            event_queue: Vec::new(),
            mouse_position: (0.0, 0.0),
            dimensions: (0, 0),
            constants: Uniforms::default(),
            enable_validation_layers: false,
            desired_validation_layer: "VK_LAYER_LUNARG_standard_validation",
        }
    }

    pub fn screen_size(&self) -> (u32, u32) {
        self.dimensions
    }

    pub fn set_uniforms(&mut self, uniforms: Uniforms) {
        self.constants = uniforms;
    }

    pub fn get_uniforms(&mut self) -> Uniforms {
        self.constants
    }

    pub fn enqueue_vertexes(&mut self, mut vertexes: Vec<Vertex>) {
        self.vertex_queue.append(&mut vertexes);
    }

    pub fn get_events(&mut self) -> Vec<Event> {
        self.event_queue.clone()
    }

    pub fn get_mouse_position(&self) -> (f64, f64) {
        self.mouse_position
    }
}

impl<D> Backend<D> for VulkanBackend
where
    D: Data,
{
    fn run(
        mut self,
        mut update_chain: UpdateChain<Self, D>,
        mut render_chain: RenderChain<Self, D>,
        mut data: D,
    ) -> D {
        let instance = {
            let required_extensions = {
                use vulkano::instance::InstanceExtensions;

                let required = required_extensions();
                println!("Required extensions: {:?}", required); // Change this to trace!
                let supported = InstanceExtensions::supported_by_core().unwrap();
                println!("Supported extensions: {:?}", supported); // Change this to trace!
                let in_common = supported.intersection(&required);
                if required != in_common {
                    let missing = supported.difference(&required);
                    panic!("Missing extensions: {:?}", missing);
                }
                required
            };

            let validation_layers = {
                use vulkano::instance::layers_list;
                use vulkano::instance::LayerProperties;

                if self.enable_validation_layers {
                    let mut layers: Vec<LayerProperties> = layers_list().unwrap().collect();
                    println!("There are {} validation layers available:", layers.len());
                    for layer in layers.iter() {
                        println!(
                            "  Layer: {}, Description: {}",
                            layer.name(),
                            layer.description()
                        );
                    }

                    if layers
                        .iter()
                        .all(|layer| layer.name() != self.desired_validation_layer)
                    {
                        panic!("The layer {} is not listed. Remember that validation layers are not available for Mac yet.", self.desired_validation_layer);
                    }
                    vec![&self.desired_validation_layer]
                } else {
                    vec![]
                }
            };

            Instance::new(None, &required_extensions, validation_layers.into_iter())
                .expect("failed to create Vulkan instance")
        };

        let mut physical_devices = PhysicalDevice::enumerate(&instance);
        let physical = physical_devices.next().expect("no device available");

        let mut events_loop = EventsLoop::new();
        let window = WindowBuilder::new()
            .build_vk_surface(&events_loop, instance.clone())
            .unwrap();

        //window.window().set_cursor_state(CursorState::Grab);
        //window.window().set_fullscreen(Some(window.window().get_current_monitor()));

        let mut dimensions = {
            let (width, height) = window.window().get_inner_size().unwrap().into();
            self.dimensions = (width, height);
            [width, height]
        };

        let queue_family = physical
            .queue_families()
            .find(|&qf| qf.supports_graphics() && window.is_supported(qf).unwrap_or(false))
            .expect("couldn't find a graphical queue family");

        let (device, mut queues) = {
            let device_ext = DeviceExtensions {
                khr_swapchain: true,
                ..DeviceExtensions::none()
            };

            //eprintln!("Supported features: {:?}", physical.supported_features());

            Device::new(
                physical,
                physical.supported_features(),
                &device_ext,
                [(queue_family, 0.5)].iter().cloned(),
            ).expect("failed to create device")
        };
        let queue = queues.next().unwrap();

        let (mut swapchain, mut images) = {
            let caps = window
                .capabilities(physical)
                .expect("failed to get surface capabilities");
            let alpha = caps.supported_composite_alpha.iter().next().unwrap();
            dimensions = caps.current_extent.unwrap_or(dimensions);
            self.dimensions = (dimensions[0], dimensions[1]);

            let format = caps.supported_formats[0].0;
            Swapchain::new(
                device.clone(),
                window.clone(),
                caps.min_image_count,
                format,
                dimensions,
                1,
                caps.supported_usage_flags,
                &queue,
                SurfaceTransform::Identity,
                alpha,
                PresentMode::Fifo,
                true,
                None,
            ).expect("failed to create swapchain")
        };

        let vs = shaders::vs::Shader::load(device.clone()).expect("failed to create shader module");
        let fs = shaders::fs::Shader::load(device.clone()).expect("failed to create shader module");

        //eprintln!("swapchain format {:?}", swapchain.format());

        let render_pass = Arc::new(
            single_pass_renderpass!(device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.format(),
                    samples: 1,
                },
                 depth: {
                    load: Clear,
                    store: DontCare,
                    format: Format::D16Unorm,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {depth}
            }
        ).unwrap(),
        );

        let pipeline = Arc::new(
            GraphicsPipeline::start()
                .vertex_input_single_buffer()
                .vertex_shader(vs.main_entry_point(), ())
                .triangle_list()
                .viewports_dynamic_scissors_irrelevant(1)
                //.cull_mode_back()
                .depth_stencil_simple_depth()
                .fragment_shader(fs.main_entry_point(), ())
                .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
                .blend_alpha_blending()
                .build(device.clone())
                .unwrap(),
        );

        // let descriptor_set = Arc::new(PersistentDescriptorSet::start(pipeline.clone(), 0)
        //     .add_buffer(data_buffer.clone()).unwrap()
        //     .build().unwrap()
        // );

        let mut framebuffers: Option<Vec<Arc<Framebuffer<_, _>>>> = None;
        let mut previous_frame_end = Box::new(now(device.clone())) as Box<GpuFuture>;
        let mut recreate_swapchain = false;

        loop {
            update_chain.update(&mut self, &mut data);
            render_chain.render(&mut self, &data);

            previous_frame_end.cleanup_finished();

            let vertex_buffer = {
                CpuAccessibleBuffer::from_iter(
                    device.clone(),
                    BufferUsage::all(),
                    self.vertex_queue.drain(..),
                ).expect("failed to create buffer")
            };

            if recreate_swapchain {
                dimensions = {
                    let (new_width, new_height) = window.window().get_inner_size().unwrap().into();
                    self.dimensions = (new_width, new_height);
                    [new_width, new_height]
                };

                let (new_swapchain, new_images) =
                    match swapchain.recreate_with_dimension(dimensions) {
                        Ok(r) => r,
                        Err(SwapchainCreationError::UnsupportedDimensions) => {
                            continue;
                        }
                        Err(err) => panic!("{:?}", err),
                    };

                mem::replace(&mut swapchain, new_swapchain);
                mem::replace(&mut images, new_images);

                framebuffers = None;

                recreate_swapchain = false;
            }

            if framebuffers.is_none() {
                let new_framebuffers = Some(
                    images
                        .iter()
                        .map(|image| {
                            let attachment_usage = ImageUsage {
                                transient_attachment: true,
                                input_attachment: false,
                                ..ImageUsage::none()
                            };
                            let img_dims = ImageAccess::dimensions(&image).width_height();
                            let depth_buffer = AttachmentImage::with_usage(
                                queue.device().clone(),
                                img_dims,
                                Format::D16Unorm,
                                attachment_usage,
                            ).unwrap();

                            Arc::new(
                                Framebuffer::start(render_pass.clone())
                                    .add(image.clone())
                                    .unwrap()
                                    .add(depth_buffer.clone())
                                    .unwrap()
                                    .build()
                                    .unwrap(),
                            )
                        })
                        .collect::<Vec<_>>(),
                );
                mem::replace(&mut framebuffers, new_framebuffers);
            }

            let (image_num, acquire_future) =
                match swapchain::acquire_next_image(swapchain.clone(), None) {
                    Ok(r) => r,
                    Err(AcquireError::OutOfDate) => {
                        recreate_swapchain = true;
                        continue;
                    }
                    Err(err) => panic!("{:?}", err),
                };

            //eprintln!(" constants: {:?}", self.constants);
            let dynamic_state = DynamicState {
                line_width: None,
                viewports: Some(vec![Viewport {
                    origin: [0.0, 0.0],
                    dimensions: [dimensions[0] as f32, dimensions[1] as f32],
                    depth_range: 0.0..1.0,
                }]),
                scissors: None,
            };

            let command_buffer =
                AutoCommandBufferBuilder::primary_one_time_submit(device.clone(), queue.family())
                    .unwrap()
                    .begin_render_pass(
                        framebuffers.as_ref().unwrap()[image_num].clone(),
                        false,
                        vec![[0.1, 0.1, 0.1, 1.0].into(), 1.0f32.into()],
                    )
                    .unwrap()
                    .draw(
                        pipeline.clone(),
                        dynamic_state,
                        vertex_buffer.clone(),
                        (),
                        self.constants,
                    )
                    .unwrap()
                    .end_render_pass()
                    .unwrap()
                    .build()
                    .unwrap();

            let future = previous_frame_end
                .join(acquire_future)
                .then_execute(queue.clone(), command_buffer)
                .unwrap()
                .then_swapchain_present(queue.clone(), swapchain.clone(), image_num)
                .then_signal_fence_and_flush()
                .unwrap();
            previous_frame_end = Box::new(future) as Box<_>;

            self.event_queue.clear();

            let mut done = false;
            events_loop.poll_events(|ev| {
                //eprintln!("{:?}", ev);
                self.event_queue.push(ev.clone());
                match ev {
                    Event::WindowEvent { event, .. } => {
                        match event {
                            WindowEvent::Closed => done = true,
                            WindowEvent::Resized(_, _) => recreate_swapchain = true,
                            WindowEvent::CursorMoved { position, .. } => {
                                self.mouse_position = position.into();
                            },
                            _ => (),
                        }
                    },
                    _ => (),
                }
            });
        }

        data
    }

    fn quit(&mut self) {
        panic!("A delicate exit");
    }
}
