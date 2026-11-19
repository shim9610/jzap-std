// Reproduction only. ?Sized and Deref are already implemented so failures cannot
// be attributed to the product's presently Sized-only Box or missing deref.
pub struct Custom<T: ?Sized>(pub Box<T>);
impl<T: ?Sized> std::ops::Deref for Custom<T> {
    type Target = T;
    fn deref(&self) -> &T { &self.0 }
}
impl<T: ?Sized> std::ops::DerefMut for Custom<T> {
    fn deref_mut(&mut self) -> &mut T { &mut self.0 }
}
