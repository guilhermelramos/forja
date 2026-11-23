fn view(ui: &mut Ui) {
    ui.button("go"); ui . label ("hello");
    ui.label(format!("ui.label( add_space(3)"));
    Color32::from_rgb(1, 2, 3);
    ui.add_space(4.0); ui.add_space(8);
    ui.add_space(variable); ui.add_space(4.0 * scale);
    // ui-lint: allow terminal content palette
    Color32::from_rgb(10, 20, 30);
    // ui.button("comment");
    /* ui.label("block"); */
}
#[cfg(test)]
mod tests { fn example() { ui.label("test"); ui.button("test"); } }
