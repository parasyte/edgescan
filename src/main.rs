use edgescan::config::Config;
use edgescan::framework::Framework;
use error_iter::ErrorIter;
use log::error;
use std::process::ExitCode;
use thiserror::Error;
use winit::dpi::LogicalSize;
use winit::event::{Event, StartCause};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

#[derive(Debug, Error)]
enum Error {
    #[error("Unable to create window")]
    Window(#[from] winit::error::OsError),

    #[error("Configuration error")]
    Config(#[from] edgescan::config::Error),
}

impl ErrorIter for Error {}

fn run() -> Result<(), Error> {
    let config = Config::new()?;
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let (width, height) = config.get_window_size();

        WindowBuilder::new()
            .with_title("EdgeScan")
            .with_inner_size(LogicalSize::new(width, height))
            .build(&event_loop)?
    };

    let mut framework = Framework::new(&event_loop, window.scale_factor(), config);
    let mut ready = false;

    event_loop.run(move |event, _, control_flow| {
        // Handle input events
        if input.update(&event) {
            // Close events
            if input.quit() {
                if let Err(err) = framework.config().save() {
                    handle_error(Error::from(err));
                }

                *control_flow = ControlFlow::Exit;
                return;
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                framework.resize(size.width, size.height, window.scale_factor());
            }

            // Update internal state and request a redraw
            window.request_redraw();
        }

        match event {
            Event::NewEvents(StartCause::Init) => {
                // SAFETY: `window` is guaranteed to live at least as long as the
                // `event_loop` run scope.
                unsafe { framework.set_window(&window) };
                ready = true;
            }
            Event::WindowEvent { event, .. } => {
                // Update egui inputs
                // TODO: Handle repaint
                let _ = framework.handle_event(&event);
            }
            // Draw the current frame
            Event::RedrawRequested(_) => {
                // TODO: Handle repaint
                if ready {
                    let _ = framework.prepare(&window);
                    framework.render();
                }
            }
            _ => (),
        }
    });
}

fn handle_error(err: Error) {
    error!("{err}");
    for source in err.sources().skip(1) {
        error!("  Caused by: {source}");
    }

    // TODO: Make fatal errors nice
    msgbox::create("Error", &format!("{err}"), msgbox::IconType::Error).unwrap();
}

fn main() -> ExitCode {
    env_logger::init();

    match run() {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            handle_error(err);

            ExitCode::FAILURE
        }
    }
}
