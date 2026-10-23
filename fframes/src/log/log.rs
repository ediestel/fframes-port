#[macro_export]
macro_rules! log {
( $( $t:tt )* ) => {
  println!( $( $t )* );
}
}
