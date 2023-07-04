#[macro_export]
macro_rules! lib_main {
    ($x:tt) => {
        use clap::Parser;
        use std::env;

        fn main() -> anyhow::Result<()> {
            if env::var("RUST_LOG").is_err() {
                env::set_var("RUST_LOG", "info");
            }

            pretty_env_logger::init();

            $x::Args::parse().exec()
        }
    };
}
