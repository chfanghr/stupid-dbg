pub fn box_err<E>(err: E) -> Box<dyn std::error::Error + 'static>
where
    E: Into<Box<dyn std::error::Error + 'static>>,
{
    err.into()
}
