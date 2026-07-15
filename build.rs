fn main() {
    // Re-run this script if the CI environment variables change.
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-env-changed=GITHUB_RUN_NUMBER");
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");

    // Embed a short commit SHA when built by GitHub Actions.
    if let Ok(sha) = std::env::var("GITHUB_SHA") {
        let short = &sha[..sha.len().min(7)];
        println!("cargo:rustc-env=GIT_COMMIT_SHA={short}");
    }

    if let Ok(run) = std::env::var("GITHUB_RUN_NUMBER") {
        println!("cargo:rustc-env=CI_RUN_NUMBER={run}");
    }

    if let Ok(refname) = std::env::var("GITHUB_REF_NAME") {
        println!("cargo:rustc-env=GIT_REF_NAME={refname}");
    }
}
