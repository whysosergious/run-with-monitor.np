use windres::Build;

#[cfg(windows)]
extern crate windres;
fn main() {
    Build::new().compile("./resources/shelly.rc").unwrap();
}
