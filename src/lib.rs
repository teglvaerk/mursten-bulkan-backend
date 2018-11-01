extern crate mursten;
extern crate nalgebra;
#[macro_use]
extern crate log;
extern crate pretty_env_logger;
#[macro_use]
extern crate vulkano;
#[macro_use]
extern crate vulkano_shader_derive;
extern crate vulkano_win;
extern crate winit;

pub mod backend;
pub mod shaders;

pub use backend::Uniforms;
pub use backend::VulkanBackend;

// This crate should not refer to mursten_blocks directly, but it needs to know
// the core traits to interact with the camera.
// I guess that in the future those blocks will go in the mursten core. But first
// I need to understand what traits are a core part of the framework.
extern crate mursten_blocks;
mod mursten_block_implementations;

