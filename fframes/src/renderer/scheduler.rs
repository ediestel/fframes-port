use std::sync::Mutex;

/// A frame handed to a worker by the [`FrameScheduler`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameClaim {
    /// Identifies the encoded segment the frame belongs to (its first frame).
    pub segment: usize,
    pub frame: usize,
    /// No other frame of this segment will be handed out after this one.
    pub last_in_segment: bool,
}

/// The most segments a timeline is cut into, unless that would make them shorter than
/// `min_segment_frames`. Each segment is a separate encoder run. Fewer, longer segments
/// leave threads idle for longer at the end of a render, while the last ones encode, and
/// the Skia pipeline encodes up to 1.5 segments per thread at once.
const MAX_SEGMENTS: usize = 128;

/// The segments a timeline is encoded in. They depend only on the number of frames and
/// `min_segment_frames` (or on the lengths given), never on the number of workers or on how
/// fast they go.
#[derive(Debug, Clone)]
struct Segments {
    /// The first frame of every segment, then the end of the timeline.
    bounds: Vec<usize>,
}

impl Segments {
    fn new(total_frames: usize, min_segment_frames: usize) -> Self {
        let length = total_frames
            .div_ceil(MAX_SEGMENTS)
            .next_multiple_of(min_segment_frames)
            .max(min_segment_frames);
        // a last segment shorter than the minimum joins the one before it
        let count = if total_frames % length >= min_segment_frames {
            total_frames.div_ceil(length)
        } else {
            total_frames / length
        }
        .max(1);

        Self {
            bounds: (0..count)
                .map(|index| index * length)
                .chain(std::iter::once(total_frames))
                .collect(),
        }
    }

    /// Segments of the given lengths, one after the other. Empty lengths are skipped.
    fn from_lengths(lengths: &[usize]) -> Self {
        let mut bounds = vec![0];
        for &length in lengths.iter().filter(|length| **length > 0) {
            bounds.push(bounds[bounds.len() - 1] + length);
        }
        Self { bounds }
    }

    fn count(&self) -> usize {
        self.bounds.len() - 1
    }

    /// The segment `frame` is in.
    fn index(&self, frame: usize) -> usize {
        self.bounds
            .partition_point(|start| *start <= frame)
            .saturating_sub(1)
            .min(self.count().saturating_sub(1))
    }

    /// The first frame of segment `index`, the end of the timeline past the last segment.
    fn start(&self, index: usize) -> usize {
        self.bounds[index.min(self.count())]
    }
}

#[derive(Debug)]
struct Slot {
    next: usize,
    /// The start of a segment, or the end of the timeline.
    end: usize,
}

impl Slot {
    fn remaining(&self) -> usize {
        self.end - self.next
    }

    fn claim(&mut self, segments: &Segments) -> Option<FrameClaim> {
        (self.next < self.end).then(|| {
            let frame = self.next;
            self.next += 1;
            let index = segments.index(frame);
            FrameClaim {
                segment: segments.start(index),
                frame,
                last_in_segment: self.next == segments.start(index + 1),
            }
        })
    }

    /// Where another worker can take over the frames left: the segment start at or before
    /// their middle, but after the segment of the next frame.
    fn split(&self, segments: &Segments) -> Option<usize> {
        if self.remaining() == 0 {
            return None;
        }
        let first = segments.start(segments.index(self.next) + 1);
        let last = segments.start(segments.index(self.end - 1));
        (first <= last).then(|| {
            segments
                .start(segments.index(self.next + self.remaining() / 2))
                .clamp(first, last)
        })
    }
}

/// Distributes the frames of a video between rendering workers.
///
/// The timeline is cut into segments before rendering starts: at most `MAX_SEGMENTS` of the
/// same length, a multiple of `min_segment_frames`. Every segment is encoded separately, and
/// the cuts depend only on the number of frames and `min_segment_frames`, so two renders of
/// the same frames encode to the same bytes, whatever the number of workers and however
/// fast each one goes.
///
/// Every worker starts with an equal, contiguous run of segments and renders it in order,
/// which keeps per-worker caches and video decoders sequential. A worker that runs out of
/// frames:
///
/// 1. takes the second half of the segments another worker has not started yet, from the
///    worker with the most frames left;
/// 2. otherwise helps with the frames right after the ones another worker is rendering,
///    in that worker's segment, so nobody idles while the last frames render. The
///    segment writer puts such frames back in order.
///
/// Segments start on multiples of `min_segment_frames` (the GOP). Every segment opens with
/// a keyframe, and there the encoder would have placed one anyway.
#[doc(hidden)]
pub struct FrameScheduler {
    slots: Vec<Mutex<Slot>>,
    segments: Segments,
}

impl FrameScheduler {
    pub fn new(total_frames: usize, workers: usize, min_segment_frames: usize) -> Self {
        Self::with_segments(
            Segments::new(total_frames, min_segment_frames.max(1)),
            workers,
        )
    }

    /// Segments of exactly these lengths, one after the other, for frames `0..sum`. The
    /// segment cache renders only the segments it does not have; the caller maps the frames
    /// back to the video.
    pub fn with_segment_lengths(lengths: &[usize], workers: usize) -> Self {
        Self::with_segments(Segments::from_lengths(lengths), workers)
    }

    fn with_segments(segments: Segments, workers: usize) -> Self {
        let workers = workers.max(1);
        let count = segments.count();
        // workers without an initial run of segments only help the others
        let runs = workers.min(count);

        let slots = (0..workers)
            .map(|worker| {
                let (first, last) = if worker < runs {
                    (count * worker / runs, count * (worker + 1) / runs)
                } else {
                    (count, count)
                };
                Mutex::new(Slot {
                    next: segments.start(first),
                    end: segments.start(last),
                })
            })
            .collect();

        Self { slots, segments }
    }

    pub fn workers(&self) -> usize {
        self.slots.len()
    }

    /// The next frame `worker` should render, `None` once every frame was handed out.
    pub fn claim(&self, worker: usize) -> Option<FrameClaim> {
        if let Some(claim) = self.slots[worker].lock().unwrap().claim(&self.segments) {
            return Some(claim);
        }

        loop {
            // the most frames left, preferring segments another worker can take over
            let (victim, (_, remaining)) = self
                .slots
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != worker)
                .map(|(index, slot)| {
                    let slot = slot.lock().unwrap();
                    (
                        index,
                        (slot.split(&self.segments).is_some(), slot.remaining()),
                    )
                })
                .max_by_key(|(_, key)| *key)?;

            if remaining == 0 {
                return None;
            }

            let mut victim_slot = self.slots[victim].lock().unwrap();
            if victim_slot.remaining() == 0 {
                // finished since we measured it
                continue;
            }

            if let Some(start) = victim_slot.split(&self.segments) {
                let mut own = Slot {
                    next: start,
                    end: victim_slot.end,
                };
                victim_slot.end = start;
                drop(victim_slot);

                let claim = own.claim(&self.segments);
                *self.slots[worker].lock().unwrap() = own;
                return claim;
            }

            return victim_slot.claim(&self.segments);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameScheduler, MAX_SEGMENTS};
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn splits_evenly_and_respects_minimum() {
        let scheduler = FrameScheduler::new(100, 16, 24);
        assert_eq!(scheduler.workers(), 16);
        assert_eq!(scheduler.claim(0).unwrap().frame, 0);
        assert_eq!(scheduler.claim(3).unwrap().frame, 72);

        let tiny = FrameScheduler::new(3, 8, 24);
        let frames: Vec<_> = std::iter::from_fn(|| tiny.claim(5)).collect();
        assert_eq!(frames.len(), 3);
        assert!(frames.iter().all(|claim| claim.segment == 0));
        assert!(frames[2].last_in_segment);
        assert_eq!(tiny.claim(0), None);
    }

    #[test]
    fn every_frame_is_claimed_once_and_segments_are_contiguous() {
        for (total, workers, min) in [(1000, 4, 10), (97, 16, 24), (48, 3, 24), (5000, 16, 24)] {
            let scheduler = FrameScheduler::new(total, workers, min);
            let mut segments: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            let mut last_flags: BTreeMap<usize, usize> = BTreeMap::new();
            let mut done = vec![false; workers];

            // workers progress at different speeds
            let mut round = 0;
            while done.iter().any(|done| !done) {
                round += 1;
                for (worker, done) in done.iter_mut().enumerate() {
                    if *done || round % (worker % 3 + 1) != 0 {
                        continue;
                    }
                    match scheduler.claim(worker) {
                        Some(claim) => {
                            segments.entry(claim.segment).or_default().push(claim.frame);
                            if claim.last_in_segment {
                                assert!(last_flags.insert(claim.segment, claim.frame).is_none());
                            }
                        }
                        None => *done = true,
                    }
                }
            }
            assert!(segments.len() <= MAX_SEGMENTS);

            let mut all = BTreeSet::new();
            for (start, frames) in &segments {
                let mut sorted = frames.clone();
                sorted.sort_unstable();
                assert_eq!(sorted[0], *start);
                assert_eq!(start % min, 0, "segment {start} does not start on the GOP");
                assert!(sorted.windows(2).all(|w| w[1] == w[0] + 1));
                assert_eq!(last_flags[start], *sorted.last().unwrap());
                assert!(sorted.len() >= min.min(total));
                for frame in frames {
                    assert!(all.insert(*frame), "frame {frame} claimed twice");
                }
            }
            assert_eq!(all.len(), total);
        }
    }

    #[test]
    fn uneven_segments_keep_their_bounds() {
        let lengths = [5, 48, 0, 13, 72, 1, 30];
        let total: usize = lengths.iter().sum();
        let mut starts = vec![0];
        for length in lengths.iter().filter(|length| **length > 0) {
            starts.push(starts.last().unwrap() + length);
        }

        for workers in [1, 2, 3, 8] {
            let scheduler = FrameScheduler::with_segment_lengths(&lengths, workers);
            let mut segments: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            let mut done = vec![false; workers];
            let mut round = 0;
            while done.iter().any(|done| !done) {
                round += 1;
                for (worker, done) in done.iter_mut().enumerate() {
                    if *done || round % (worker % 2 + 1) != 0 {
                        continue;
                    }
                    match scheduler.claim(worker) {
                        Some(claim) => {
                            let index = starts.partition_point(|start| *start <= claim.frame) - 1;
                            assert_eq!(claim.segment, starts[index]);
                            assert_eq!(claim.last_in_segment, claim.frame + 1 == starts[index + 1]);
                            segments.entry(claim.segment).or_default().push(claim.frame);
                        }
                        None => *done = true,
                    }
                }
            }
            let claimed: BTreeSet<usize> = segments.values().flatten().copied().collect();
            assert_eq!(claimed.len(), total);
            assert_eq!(segments.len(), starts.len() - 1);
        }

        assert_eq!(FrameScheduler::with_segment_lengths(&[], 4).claim(2), None);
    }
}
