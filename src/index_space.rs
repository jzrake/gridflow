use core::ops::Range;




#[derive(Clone)]
pub struct IndexSpace2d {
    di: Range<i64>,
    dj: Range<i64>,
}




/**
 * Describes a rectangular index space. The index type is signed 64-bit integer.
 */
impl IndexSpace2d {



    pub fn new(di: Range<i64>, dj: Range<i64>) -> Self {
        Self { di, dj }
    }


    /**
     * Return the number of indexes on each axis.
     */
    pub fn dim(&self) -> (usize, usize) {
        ((self.di.end - self.di.start) as usize,
         (self.dj.end - self.dj.start) as usize)
    }


    pub fn start(&self) -> (i64, i64) {
        (self.di.start, self.dj.start)
    }


    pub fn end(&self) -> (i64, i64) {
        (self.di.end, self.dj.end)
    }


    pub fn as_rect_ref(&self) -> (&Range<i64>, &Range<i64>) {
        (&self.di, &self.dj)
    }


    pub fn contains(&self, index: (i64, i64)) -> bool {
        self.di.contains(&index.0) && self.dj.contains(&index.1)
    }


    pub fn extend_all(&self, delta: i64) -> Self {
        Self::new(
            self.di.start - delta .. self.di.end + delta,
            self.dj.start - delta .. self.dj.end + delta)
    }


    pub fn scale(&self, factor: i64) -> Self {
        Self::new(
            self.di.start * factor .. self.di.end * factor,
            self.dj.start * factor .. self.dj.end * factor)
    }


    pub fn iter(&self) -> impl Iterator<Item = (i64, i64)> + '_ {
        self.di.clone().map(move |i| self.dj.clone().map(move |j| (i, j))).flatten()
    }
}




// ============================================================================
impl IntoIterator for IndexSpace2d {
    type Item = (i64, i64);
    type IntoIter = impl Iterator<Item = Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let Self { di, dj } = self;
        di.map(move |i| dj.clone().map(move |j| (i, j))).flatten()
    }
}

impl From<(Range<i64>, Range<i64>)> for IndexSpace2d {
    fn from(range: (Range<i64>, Range<i64>)) -> Self {
        Self { di: range.0, dj: range.1 }
    }
}

impl<'a> From<(&'a Range<i64>, &'a Range<i64>)> for IndexSpace2d {
    fn from(range: (&'a Range<i64>, &'a Range<i64>)) -> Self {
        Self { di: range.0.clone(), dj: range.1.clone() }
    }
}

impl From<IndexSpace2d> for (Range<i64>, Range<i64>) {
    fn from(space: IndexSpace2d) -> Self {
        (space.di, space.dj)
    }
}




/**
 * Less imposing factory function to construct an IndexSpace2d object.
 */
pub fn range2d(di: Range<i64>, dj: Range<i64>) -> IndexSpace2d {
    IndexSpace2d::new(di, dj)
}
