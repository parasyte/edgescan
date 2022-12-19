use edgescan::{config::Config, framework::Framework, gpu::Gpu};
use error_iter::ErrorIter;
use log::error;
use std::{process::ExitCode, time::Duration};
use thiserror::Error;
use winit::{
    dpi::LogicalSize,
    event::Event,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};
use winit_input_helper::WinitInputHelper;

#[cfg(target_os = "macos")]
use std::time::Instant;

#[derive(Debug, Error)]
enum Error {
    #[error("Unable to create window")]
    Window(#[from] winit::error::OsError),

    #[error("Unable to find a suitable GPU")]
    Gpu(#[from] edgescan::gpu::Error),

    #[error("Configuration error")]
    Config(#[from] edgescan::config::Error),
}

impl ErrorIter for Error {}

fn run() -> Result<(), Error> {
    let config = Config::new()?;
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let (window, mut framework) = {
        let (width, height) = config.get_window_size();

        let window = WindowBuilder::new()
            .with_title("EdgeScan")
            .with_inner_size(LogicalSize::new(width, height))
            .build(&event_loop)?;

        // SAFETY: The window is moved into the event_loop run closure, ensuring it lives at least
        // as long as `gpu`
        let gpu = unsafe { Gpu::new(&window, window.inner_size())? };

        let framework = Framework::new(
            &event_loop,
            window.inner_size(),
            window.scale_factor(),
            config,
            gpu,
        );

        (window, framework)
    };

    let mut repaint = Duration::ZERO;

    #[cfg(target_os = "macos")]
    let mut now = Instant::now();

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
                framework.resize(size, window.scale_factor());
            }

            // Update internal state and request a redraw
            repaint = framework.prepare(&window);
            maybe_redraw(control_flow, &window, repaint.is_zero());
        }

        match event {
            Event::WindowEvent { event, .. } => {
                // Update egui inputs
                maybe_redraw(
                    control_flow,
                    &window,
                    framework.handle_event(&event).repaint,
                );
            }
            Event::RedrawRequested(_) => {
                // Draw the current frame
                if let Err(err) = framework.render() {
                    error!("framework.render() failed: {err}");
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                maybe_redraw(control_flow, &window, repaint.is_zero());
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

fn maybe_redraw(control_flow: &mut ControlFlow, window: &Window, do_it: bool) {
    if do_it {
        window.request_redraw();
        *control_flow = ControlFlow::Poll;
    } else {
        *control_flow = ControlFlow::Wait;
    }
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
