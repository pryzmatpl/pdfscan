use egui::{Context, Visuals, Color32, Stroke, Rounding};

/// Set up a custom theme for the application
pub fn setup_custom_theme(ctx: &Context) {
    // Start with the dark theme as a base
    let mut visuals = Visuals::dark();
    
    // Customize colors
    visuals.panel_fill = Color32::from_rgb(25, 25, 30);
    visuals.window_fill = Color32::from_rgb(30, 30, 35);
    
    // Active widgets have a slightly lighter background
    visuals.widgets.active.bg_fill = Color32::from_rgb(50, 50, 60);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::from_rgb(210, 210, 220));
    
    // Inactive widgets are darker
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 40, 50);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(180, 180, 190));
    
    // Selected items are highlighted with a subtle blue
    visuals.selection.bg_fill = Color32::from_rgb(0, 92, 128);
    visuals.selection.stroke = Stroke::new(1.0, Color32::from_rgb(0, 140, 230));
    
    // Hyperlinks
    visuals.hyperlink_color = Color32::from_rgb(90, 170, 255);
    
    // Rounded corners for everything
    let rounding = Rounding::same(4.0);
    visuals.window_rounding = rounding;
    visuals.menu_rounding = rounding;
    
    // Apply the custom theme
    ctx.set_visuals(visuals);
    
    // Set default fonts
    let fonts = egui::FontDefinitions::default();
    
    // Apply the fonts
    ctx.set_fonts(fonts);
} 