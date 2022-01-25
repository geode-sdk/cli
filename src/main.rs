fn main() {
    let cmd = std::env::args().nth(1).expect("Help information for geode or smth");

    println!("{}", cmd);
}
