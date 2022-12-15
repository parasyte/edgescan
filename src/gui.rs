use dwfv::signaldb::{BitValue, SignalDB, SignalValue};
use egui::{Context, Painter, Rect, Ui, Vec2};
use rfd::AsyncFileDialog;
use std::thread::JoinHandle;
use winit::window::Window;

pub struct Gui {
    enabled: bool,
    about_open: bool,
    vcd: Option<SignalDB>,
    file_dialog: Option<JoinHandle<Option<SignalDB>>>,
}

impl Gui {
    pub(crate) fn new() -> Self {
        Self {
            enabled: true,
            about_open: false,
            vcd: None,
            file_dialog: None,
        }
    }

    /// Create the UI using egui.
    pub(crate) fn ui(&mut self, ctx: &Context, window: &Window) {
        // Poll the file dialog
        if let Some(handle) = self.file_dialog.as_ref() {
            if handle.is_finished() {
                if let Ok(vcd) = self.file_dialog.take().unwrap().join() {
                    self.vcd = vcd.or(self.vcd.take());
                }
                self.enabled = true;
            }
        }

        // Draw the menu bar
        egui::TopBottomPanel::top("menubar_container").show(ctx, |ui| {
            ui.set_enabled(self.enabled);
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open").clicked() {
                        let dialog = AsyncFileDialog::new()
                            .set_parent(window)
                            .add_filter("Value Change Dump", &["vcd"]);

                        self.file_dialog = Some(std::thread::spawn(move || {
                            pollster::block_on(dialog.pick_file())
                                .and_then(|handle| std::fs::read(handle.path()).ok())
                                .and_then(|buf| SignalDB::from_vcd(&buf[..]).ok())
                        }));
                        self.enabled = false;

                        ui.close_menu();
                    }

                    if self.vcd.is_some() && ui.button("Close").clicked() {
                        self.vcd = None;
                        ui.close_menu();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About...").clicked() {
                        self.about_open = true;
                        ui.close_menu();
                    }
                });
            });
        });

        // Draw the main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_enabled(self.enabled);
            if let Some(_vcd) = self.vcd.as_ref() {
                self.draw_vcd(ui);
            }
        });

        // Draw the windows (if requested by the user)
        self.about_window(ctx);
    }

    /// Show "About" window.
    fn about_window(&mut self, ctx: &Context) {
        egui::Window::new("About EdgeScan")
            .open(&mut self.about_open)
            .enabled(self.enabled)
            .collapsible(false)
            .default_pos((175.0, 175.0))
            .fixed_size((350.0, 100.0))
            .show(ctx, |ui| {
                ui.add_space(5.0);
                ui.label(concat!("EdgeScan version ", env!("CARGO_PKG_VERSION")));
                ui.add_space(10.0);
                ui.label(env!("CARGO_PKG_DESCRIPTION"));
                ui.label(concat!("By: ", env!("CARGO_PKG_AUTHORS")));
                ui.add_space(10.0);
                ui.label("Made with 💖 in San Francisco!");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label("Website:");
                    ui.hyperlink(env!("CARGO_PKG_HOMEPAGE"));
                });
            });
    }

    /// Draw the VCD waveforms.
    fn draw_vcd(&self, ui: &mut Ui) {
        let vcd = self.vcd.as_ref().unwrap();
        let signals: Vec<_> = vcd
            .get_signal_ids()
            .into_iter()
            .map(|id| (vcd.get_signal_fullname(&id).unwrap(), id))
            .collect();

        let sense = egui::Sense::hover();
        let size = get_max_string_size(ui, signals.iter().map(|(name, _)| name));

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            // TODO: use `show_viewport` and manually clip the samples drawn
            .show(ui, |ui| {
                for (i, (name, id)) in signals.iter().enumerate() {
                    ui.horizontal(|ui| {
                        // Allocate space for the fixed signal name column
                        let (mut rect, _) = ui.allocate_exact_size(size, sense);
                        let spacing_x = ui.spacing().item_spacing.x;

                        let bg_color = if i % 2 == 0 {
                            ui.style().visuals.window_fill
                        } else {
                            ui.style().visuals.faint_bg_color
                        };

                        // Draw background for waveform column
                        // TODO: Only draw the odd rows
                        // Should also draw the full row all the way across all columns.
                        {
                            let rect = Rect::from_min_size(
                                rect.right_top(),
                                Vec2::new(f32::INFINITY, rect.max.y),
                            );
                            ui.painter().rect_filled(rect.expand(3.0), 0.0, bg_color);
                        }

                        // Draw waveform
                        // TODO: Draw a timeline header
                        {
                            let zoom = 35.0; // TODO: Zoom with CTRL + Mousewheel
                            let sample_size = Vec2::new(zoom, size.y);
                            for ts in vcd.get_timestamps() {
                                let (mut rect, _) = ui.allocate_exact_size(sample_size, sense);
                                rect.set_width(zoom + spacing_x);
                                draw_waveform_sample(
                                    ui.painter(),
                                    rect,
                                    vcd.value_at(id, ts).unwrap(),
                                );
                            }
                        }

                        // Draw background for signal name column
                        // TODO: Only draw the odd rows
                        // Needs clipping on the waveform to avoid overdraw.
                        let painter = ui.painter();
                        rect.min.x = 0.0;
                        rect.max.x = spacing_x + size.x;
                        painter.rect_filled(rect.expand(3.0), 0.0, bg_color);

                        // Draw signal name with fixed X position and width
                        let text_galley = egui::WidgetText::from(name)
                            .into_text_job(
                                ui.style(),
                                egui::FontSelection::Default,
                                egui::Align::LEFT,
                            )
                            .into_galley(&ui.fonts());
                        rect.min.x = spacing_x;
                        painter.galley_with_color(
                            rect.min,
                            text_galley.galley,
                            ui.style().visuals.text_color(),
                        );
                    });
                }
            });
    }
}

fn get_max_string_size<'a>(ui: &Ui, strings: impl Iterator<Item = &'a String>) -> Vec2 {
    let spacing = ui.spacing();

    strings.fold(Vec2::ZERO, |width, text| {
        let galley = ui.fonts().layout_no_wrap(
            text.to_string(),
            egui::TextStyle::Body.resolve(ui.style()),
            egui::Color32::TEMPORARY_COLOR,
        );

        width.max(Vec2::new(
            galley.rect.width() + spacing.item_spacing.x,
            galley.rect.height(),
        ))
    })
}

fn draw_waveform_sample(painter: &Painter, rect: Rect, sample: SignalValue) {
    let stroke = (1.0, egui::Color32::GREEN);

    match sample {
        SignalValue::Literal(bits, _) => {
            if bits.len() == 1 {
                match bits[0] {
                    // TODO: Use paths instead of line segments?
                    BitValue::Low => {
                        painter.line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
                    }
                    BitValue::High => {
                        painter.line_segment([rect.left_top(), rect.right_top()], stroke);
                    }
                    BitValue::HighZ => {
                        // TODO
                        painter.line_segment([rect.left_top(), rect.right_top()], stroke);
                        painter.line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
                    }
                    _ => {
                        // TODO
                        painter.rect_filled(rect, 0.0, egui::Color32::RED);
                    }
                }
            } else {
                // TODO
                painter.line_segment([rect.left_top(), rect.right_top()], stroke);
                painter.line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
            }
        }
        SignalValue::Symbol(_) => (),
    }
}
