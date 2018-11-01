mod camera {
    use backend::{VulkanBackend, Uniforms};
    use nalgebra::*;

    use mursten_blocks::camera::Camera;
    use mursten_blocks::camera::backend::SetCamera;

    impl SetCamera for VulkanBackend {
        fn set_camera(&mut self, transform: Matrix4<f32>, camera: &Camera) {
            self.set_uniforms(Uniforms {
                projection_view: camera.projection.clone() * transform,
                ..Uniforms::default()
            });
        }
    }
}

mod render {
    use backend;
    use nalgebra::*;
    use mursten_blocks::mesh_renderer::backend::RenderMesh;
    use mursten_blocks::geometry::{Mesh, Triangle, Vertex};

    impl RenderMesh for backend::VulkanBackend {
        fn queue_render(&mut self, m: Matrix4<f32>, mesh: Mesh) {
            let vertexes = mesh.transform(&m).triangles.into_iter().fold(Vec::new(), |mut vs, t| {
                let Triangle { v1, v2, v3 } = t;

                let n1 = (v1.position - v3.position).cross(&(v1.position - v2.position)); // Le podes haber pifiado a la dirección de la normal
                vs.push((n1, v1).into());

                let n2 = (v2.position - v1.position).cross(&(v2.position - v3.position)); // Le podes haber pifiado a la dirección de la normal
                vs.push((n2, v2).into());

                let n3 = (v3.position - v2.position).cross(&(v3.position - v1.position)); // Le podes haber pifiado a la dirección de la normal
                vs.push((n3, v3).into());
                vs
            });
            self.enqueue_vertexes(vertexes);
        }
    }

    impl From<(Vector3<f32>, Vertex)> for backend::Vertex {
        fn from(pair: (Vector3<f32>, Vertex)) -> backend::Vertex {
            let (n, v) = pair;
            backend::Vertex {
                position: v.position.to_homogeneous().into(),
                normal: n.to_homogeneous().into(),
                color: v.color.into(),
                texture: [v.texture.x, v.texture.y],
            }
        }
    }
}

mod light {
    use backend;
    use mursten_blocks::light::Light;
    use mursten_blocks::light::backend::SetLights;
    use nalgebra::*;

    impl SetLights for backend::VulkanBackend {
        fn set_light(&mut self, light: Light) {
            let Light { point, color, strength } = light;
            let mut uniforms = self.get_uniforms();
            uniforms.light_origin = Vector4::new(point.x, point.y, point.z, 1.0);
            uniforms.light_color = Vector4::new(color.x, color.y, color.z, 1.0);
            uniforms.ambient_light_strength = strength;
            uniforms.diffuse_light_strength = strength;
            uniforms.specular_light_strength = strength;
            self.set_uniforms(uniforms);
        }
    }
}

mod input {
    use backend;
    use mursten_blocks::input::{Key, KeyModifiers, KeyboardEvent, MouseEvent, MouseButton};
    use mursten_blocks::input::backend::{KeyboardEventSource, MouseEventSource};
    use winit;
    use nalgebra::*;

    impl KeyboardEventSource for backend::VulkanBackend {
        fn drain_events(&mut self) -> Vec<KeyboardEvent> {
           self.get_events().into_iter().filter_map(|event| {

               match event {
                   winit::Event::WindowEvent { event, .. } => Some(event),
                   _ => None,
               }

           }).filter_map(|window_event| {

               match window_event {
                    winit::WindowEvent::KeyboardInput { input, .. } => Some(input),
                    _ => None,
                }

           }).filter_map(|keyboard_input| {

                let key = keyboard_input.virtual_keycode.map(|vk| match vk {
                    winit::VirtualKeyCode::A => Some(Key::A),
                    winit::VirtualKeyCode::S => Some(Key::S),
                    winit::VirtualKeyCode::D => Some(Key::D),
                    winit::VirtualKeyCode::Q => Some(Key::Q),
                    winit::VirtualKeyCode::W => Some(Key::W),
                    winit::VirtualKeyCode::E => Some(Key::E),
                    winit::VirtualKeyCode::J => Some(Key::J),
                    winit::VirtualKeyCode::K => Some(Key::K),
                    winit::VirtualKeyCode::F => Some(Key::F),
                    _ => None
                })??;
                let modifiers = KeyModifiers {};

                let event = match keyboard_input.state {
                    winit::ElementState::Pressed => KeyboardEvent::Pressed(key, modifiers),
                    winit::ElementState::Released => KeyboardEvent::Released(key, modifiers),
                };
                Some(event)

            }).collect()
        }
    }

    impl MouseEventSource for backend::VulkanBackend {
        fn drain_events(&mut self) -> Vec<MouseEvent> {
           let mut mouse_events_from_device: Vec<MouseEvent> = self.get_events().into_iter().filter_map(|event| {
               match event {
                   winit::Event::DeviceEvent { event, .. } => Some(event),
                   _ => None,
               }
           }).filter_map(|device_event| {

               match device_event {
                    winit::DeviceEvent::MouseMotion { delta, .. } => {
                        Some(MouseEvent::Movement(Vector2::new(delta.0 as f32, delta.1 as f32)))
                    },
                    winit::DeviceEvent::MouseWheel { delta, .. } => {
                        const LINE_SIZE: f32 = 1.0;
                        Some(MouseEvent::Wheel(
                            match delta {
                                winit::MouseScrollDelta::LineDelta(h, v) => Vector2::new(h, v) * LINE_SIZE,
                                winit::MouseScrollDelta::PixelDelta(h, v) => Vector2::new(h, v),
                            }
                        ))
                    },
                    winit::DeviceEvent::Button { state, button, .. } => {
                        let button = match button {
                            _ => MouseButton::Left,
                        };
                        let position = self.get_mouse_position();
                        let position = Point2::new(position.0 as f32, position.1 as f32);
                        match state {
                            winit::ElementState::Pressed => Some(MouseEvent::Pressed(button, position)),
                            winit::ElementState::Released => Some(MouseEvent::Released(button, position)),
                        }
                    }
                    _ => None,
                }

            }).collect();

           let mut mouse_events_from_window: Vec<MouseEvent> = self.get_events().into_iter().filter_map(|event| {
               match event {
                   winit::Event::WindowEvent { event, .. } => Some(event),
                   _ => None,
               }
           }).filter_map(|window_event| {

               match window_event {
                    winit::WindowEvent::MouseInput { state, button, .. } => {
                        let button = match button {
                            _ => MouseButton::Left,
                        };
                        let position = self.get_mouse_position();
                        let position = Point2::new(position.0 as f32, position.1 as f32);
                        match state {
                            winit::ElementState::Pressed => Some(MouseEvent::Pressed(button, position)),
                            winit::ElementState::Released => Some(MouseEvent::Released(button, position)),
                        }
                    }
                    _ => None,
                }

            }).collect();

           let is_mouse_pressed = |ev: &MouseEvent| match *ev { MouseEvent::Pressed(_, _) => true, _ => false };
           if mouse_events_from_device.iter().any(is_mouse_pressed) {
                mouse_events_from_window.retain(|ev| !is_mouse_pressed(ev));
           }

            mouse_events_from_device.append(&mut mouse_events_from_window);
            mouse_events_from_device
        }
    }
}
