#[cfg(not(test))]
fn main() -> anyhow::Result<()> {
    simengine::cli::run()
}

#[cfg(test)]
fn main() {}
