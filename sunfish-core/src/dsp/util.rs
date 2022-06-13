/// Find the closest frequency, biased either up or down.
pub fn closest_number_in(search: f64, freqs: &[f64], bias_up: bool) -> f64 {
    // Variation on binary search where we account for items in the range between points. To
    // accommodate this, we vary from traditional binary search by moving the first and last
    // markers to *inclusive* points.
    let n = freqs.len();
    if n == 0 {
        return 0.0;
    }

    let mut first = 0;
    let mut last = n - 1;
    let mut middle = n / 2;
    if search < freqs[first] {
        return freqs[first];
    }
    if search > freqs[last] {
        return freqs[last];
    }

    while last - first > 1 {
        let mid_value = freqs[middle];
        #[allow(clippy::float_cmp)]
        if search == mid_value {
            return mid_value;
        } else if search > mid_value {
            first = middle;
        } else {
            last = middle;
        }

        middle = (first + last) / 2;
    }

    let (i, j) = if bias_up {
        (last, first)
    } else {
        (first, last)
    };
    #[allow(clippy::float_cmp)]
    if freqs[i] == search {
        freqs[i]
    } else {
        freqs[j]
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[allow(clippy::float_cmp)]
    #[test]
    fn lookup_freq() {
        let fs = [0.0, 5.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0];
        assert_eq!(closest_number_in(1.0, &fs, true), 0.0);
        assert_eq!(closest_number_in(4.0, &fs, true), 0.0);
        assert_eq!(closest_number_in(5.0, &fs, true), 5.0);
        assert_eq!(closest_number_in(16.0, &fs, true), 15.0);

        assert_eq!(closest_number_in(1.0, &fs, false), 5.0);
        assert_eq!(closest_number_in(4.0, &fs, false), 5.0);
        assert_eq!(closest_number_in(5.0, &fs, false), 5.0);
        assert_eq!(closest_number_in(16.0, &fs, false), 15.0);

        let fs = [0.0, 5.0, 10.0];
        assert_eq!(closest_number_in(1.0, &fs, true), 0.0);
        assert_eq!(closest_number_in(4.0, &fs, true), 0.0);
        assert_eq!(closest_number_in(5.0, &fs, true), 5.0);
        assert_eq!(closest_number_in(6.0, &fs, true), 5.0);
        assert_eq!(closest_number_in(10.0, &fs, true), 10.0);
        assert_eq!(closest_number_in(12.0, &fs, true), 10.0);

        assert_eq!(closest_number_in(0.0, &fs, false), 0.0);
        assert_eq!(closest_number_in(1.0, &fs, false), 5.0);
        assert_eq!(closest_number_in(4.0, &fs, false), 5.0);
        assert_eq!(closest_number_in(5.0, &fs, false), 5.0);
        assert_eq!(closest_number_in(6.0, &fs, false), 10.0);
        assert_eq!(closest_number_in(10.0, &fs, false), 10.0);
        assert_eq!(closest_number_in(12.0, &fs, false), 10.0);

        let fs = [5.0, 10.0];
        assert_eq!(closest_number_in(1.0, &fs, true), 5.0);
        assert_eq!(closest_number_in(1.0, &fs, false), 5.0);
    }
}

