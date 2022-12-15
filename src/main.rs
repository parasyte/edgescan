use edgescan::{config::Config, framework::Framework};
use error_iter::ErrorIter;
use log::error;
use std::process::ExitCode;
use thiserror::Error;
use winit::{
    dpi::LogicalSize,
    event::{Event, StartCause},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};

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

    #[cfg(target_os = "macos")]
    let mut now = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        // Wait for the next event
        *control_flow = ControlFlow::Wait;

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
            if framework.prepare(&window).is_zero() {
                window.request_redraw();
            }
        }

        match event {
            Event::NewEvents(StartCause::Init) => {
                // SAFETY: `window` is guaranteed to live at least as long as the
                // `event_loop` run scope.
                unsafe { framework.set_window(&window) };
            }
            Event::WindowEvent { event, .. } => {
                // Update egui inputs
                if framework.handle_event(&event).repaint {
                    window.request_redraw();
                }
            }
            Event::RedrawRequested(_) => {
                // Draw the current frame
                framework.render();
            }
            Event::RedrawEventsCleared => {
                // TODO: `ControlFlow::Wait` doesn't work on macOS.
                // See: https://github.com/rust-windowing/winit/issues/1985
                #[cfg(target_os = "macos")]
                {
                    let target = Duration::from_secs_f64(1.0 / 60.0);
                    let actual = now.elapsed();
                    if target > actual {
                        std::thread::sleep(target - actual);
                    }
                    now = Instant::now();
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
