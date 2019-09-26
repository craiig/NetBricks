use e2d2::headers::*;
use e2d2::operators::*;
use std::ptr::write_volatile;

pub fn macswap<T: 'static + Batch<Header = NullHeader>>(
    parent: T,
    spin: u64
) -> TransformBatch<MacHeader, ParsedBatch<MacHeader, T>> {
    parent.parse::<MacHeader>().transform(box move |pkt| {
        assert!(pkt.refcnt() == 1);
        let hdr = pkt.get_mut_header();
        hdr.swap_addresses();

	// https://rust.godbolt.org shows this won't get optimized out
    	let mut sum: u64 = 0;
    	let y = &mut sum as *mut u64;
    	for x in 0..spin {
	    unsafe { write_volatile(y, x+*y); }
    	}
    })
}
