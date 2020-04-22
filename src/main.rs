use image;

use metal::*;

use winit::platform::macos::WindowExtMacOS;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

use cocoa::foundation::NSUInteger;
use cocoa::{appkit::NSView, base::id as cocoa_id};

use core_graphics::geometry::CGSize;
use objc::runtime::YES;

use std::mem;

// Declare the data structures needed to carry vertex layout to
// metal shading language(MSL) program. Use #[repr(C)], to make
// the data structure compatible with C++ type data structure
// for vertex defined in MSL program as MSL program is broadly
// based on C++
#[repr(C)]
#[derive(Debug)]
pub struct position(cty::c_float, cty::c_float);
#[repr(C)]
#[derive(Debug)]
pub struct texture_coordinate(cty::c_float, cty::c_float);
#[repr(C)]
#[derive(Debug)]
pub struct AAPLVertex {
    p: position,
    t: texture_coordinate,
}

fn prepare_render_pass_descriptor(descriptor: &RenderPassDescriptorRef, texture: &TextureRef) {
    let color_attachment = descriptor.color_attachments().object_at(0).unwrap();

    color_attachment.set_texture(Some(texture));
    color_attachment.set_load_action(MTLLoadAction::Clear);
    // Setting a background color
    color_attachment.set_clear_color(MTLClearColor::new(0.5, 0.5, 0.8, 0.1));
    color_attachment.set_store_action(MTLStoreAction::Store);
}

fn prepare_pipeline_state(device: &DeviceRef, library: &Library) -> RenderPipelineState {
    let vert = library.get_function("vertexShader", None).unwrap();
    let frag = library.get_function("samplingShader", None).unwrap();

    let pipeline_state_descriptor = RenderPipelineDescriptor::new();
    pipeline_state_descriptor.set_vertex_function(Some(&vert));
    pipeline_state_descriptor.set_fragment_function(Some(&frag));
    pipeline_state_descriptor
        .color_attachments()
        .object_at(0)
        .unwrap()
        .set_pixel_format(MTLPixelFormat::BGRA8Unorm);

    device
        .new_render_pipeline_state(&pipeline_state_descriptor)
        .unwrap()
}

fn prepare_texture_from_file(device: &DeviceRef, source: &str) -> Texture {
    let image = image::open(source);
    let image_buffer = image.unwrap().into_rgba();
    let width: u64 = image_buffer.width().into();
    let height: u64 = image_buffer.height().into();
    println!("Height {} and width are {}", height, width);
    let td = TextureDescriptor::new();
    td.set_width(width);
    td.set_height(height);
    td.set_pixel_format(MTLPixelFormat::BGRA8Unorm);

    let texture: Texture = device.new_texture(&td);
    let reg = MTLRegion {
        origin: MTLOrigin { x: 0, y: 0, z: 0 },
        size: MTLSize {
            width: width,
            height: height,
            depth: 1,
        },
    };
    let bytes_per_row: NSUInteger = 4 * width;
    let l = image_buffer.into_raw();
    println!("The image bytes length {}", &l.len());
    texture.replace_region(reg, 0, bytes_per_row, l.as_ptr() as *const std::ffi::c_void);

    return texture;
}

fn main() {
    // Create a window for viewing the content
    let event_loop = EventLoop::new();
    let events_loop = winit::event_loop::EventLoop::new();
    let size = winit::dpi::LogicalSize::new(800, 600);

    let window = winit::window::WindowBuilder::new()
        .with_inner_size(size)
        .with_title("Sampling Textures".to_string())
        .build(&events_loop)
        .unwrap();

    // Set up the GPU device found in the system
    let device = Device::system_default().expect("no device found");
    println!("Your device is: {}", device.name(),);

    // Set the command queue used to pass commands to the device.
    let command_queue = device.new_command_queue();

    // Currently, CoreAnimationLayer is the only interface that provide
    // layers to carry drawable texture from GPU rendaring through metal
    // library to viewable windows.
    let layer = CoreAnimationLayer::new();
    layer.set_device(&device);
    layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    layer.set_presents_with_transaction(false);

    unsafe {
        let view = window.ns_view() as cocoa_id;
        view.setWantsLayer(YES);
        view.setLayer(mem::transmute(layer.as_ref()));
    }

    let draw_size = window.inner_size();
    layer.set_drawable_size(CGSize::new(draw_size.width as f64, draw_size.height as f64));

    let vbuf = {
        //let vertex_data = create_vertex_points_for_circle();
        //let vertex_data = vertex_data.as_slice();
        let vertex_data = [
            AAPLVertex {
                p: position(1.0, -1.0),
                t: texture_coordinate(1.0, 1.0),
            },
            AAPLVertex {
                p: position(1.0, 1.0),
                t: texture_coordinate(1.0, 0.0),
            },
            AAPLVertex {
                p: position(-1.0, -1.0),
                t: texture_coordinate(0.0, 1.0),
            },
            AAPLVertex {
                p: position(-1.0, -1.0),
                t: texture_coordinate(0.0, 1.0),
            },
            AAPLVertex {
                p: position(1.0, 1.0),
                t: texture_coordinate(1.0, 0.0),
            },
            AAPLVertex {
                p: position(-1.0, 1.0),
                t: texture_coordinate(0.0, 0.0),
            },
        ];

        device.new_buffer_with_data(
            vertex_data.as_ptr() as *const _,
            (vertex_data.len() * mem::size_of::<AAPLVertex>()) as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache | MTLResourceOptions::StorageModeManaged,
        )
    };

    // Use the metallib file generated out of .metal shader file
    let library = device.new_library_with_file("shaders.metallib").unwrap();

    // The render pipeline generated from the vertex and fragment shaders in the .metal shader file.
    let pipeline_state = prepare_pipeline_state(&device, &library);

    // Set the texture here
    let tref = prepare_texture_from_file(&device, "Image.tga");

    event_loop.run(move |event, _, control_flow| {
        // ControlFlow::Wait pauses the event loop if no events are available to process.
        // This is ideal for non-game applications that only update in response to user
        // input, and uses significantly less power/CPU time than ControlFlow::Poll.
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                println!("The close button was pressed; stopping");
                *control_flow = ControlFlow::Exit
            }
            Event::MainEventsCleared => {
                // Queue a RedrawRequested event.
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                // It's preferrable to render in this event rather than in MainEventsCleared, since
                // rendering in here allows the program to gracefully handle redraws requested
                // by the OS.
                let drawable = match layer.next_drawable() {
                    Some(drawable) => drawable,
                    None => return,
                };

                // Obtain a renderPassDescriptor generated from the view's drawable textures.
                let render_pass_descriptor = RenderPassDescriptor::new();
                prepare_render_pass_descriptor(&render_pass_descriptor, drawable.texture());

                // Create a new command buffer for each render pass to the current drawable
                let command_buffer = command_queue.new_command_buffer();

                // Create a render command encoder.
                let encoder = command_buffer.new_render_command_encoder(&render_pass_descriptor);
                encoder.set_render_pipeline_state(&pipeline_state);
                // Pass in the parameter data.
                encoder.set_vertex_buffer(0, Some(&vbuf), 0);
                encoder.set_fragment_texture(0, Some(&tref));
                // Draw the triangles which will eventually form the circle.
                encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 6);
                encoder.end_encoding();

                // Schedule a present once the framebuffer is complete using the current drawable.
                command_buffer.present_drawable(&drawable);

                // Finalize rendering here & push the command buffer to the GPU.
                command_buffer.commit();
            }
            _ => (),
        }
    });
}
