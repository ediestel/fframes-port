//! Pre-render validation: checks the video configuration for common errors before the pipeline
//! starts, so they fail with a clear message instead of after frames were already encoded.
use crate::Duration;
use crate::MediaProvider;
use crate::error::{FFramesError, Result};
use crate::scenes::{Overlap, Scenes};

/// Validates a video's configuration before rendering begins.
///
/// Checks performed:
/// 1. The top-level `Duration` is resolvable: `Duration::Auto` needs scenes or media.
/// 2. Every `Duration::FromAudio` reference, at the top level and in each scene's audio map,
///    resolves in the media provider.
/// 3. A scene's overlaps do not exceed its own duration, where that duration is
///    `Duration::Seconds` and therefore known statically.
///
/// Returns `Ok(())` if all checks pass, or the first `FFramesError` found.
pub fn validate_video<'a, 'm>(
    duration: &Duration<'a>,
    scenes: &Scenes<'a>,
    media: Option<&'m dyn MediaProvider<'m>>,
) -> Result<()> {
    if matches!(duration, Duration::Auto) && scenes.is_empty() && media.is_none() {
        return Err(FFramesError::MissingDurationOrScenes);
    }

    let require_audio = |name: &str| -> Result<()> {
        match media.and_then(|m| m.resolve_audio(name)) {
            Some(_) => Ok(()),
            None => Err(FFramesError::RequiredAudioNotFound(name.to_owned())),
        }
    };

    if let Some(audio_files) = duration.used_audio_files() {
        for name in audio_files {
            require_audio(name)?;
        }
    }

    let Some(scene_list) = scenes.0.as_ref() else {
        return Ok(());
    };

    for scene in scene_list.iter() {
        if let Some(audio_files) = scene.audio().used_audio_files::<Vec<&str>>() {
            for name in audio_files {
                require_audio(name)?;
            }
        }

        let Duration::Seconds(scene_secs) = scene.duration() else {
            continue;
        };
        let (previous, next) = match scene.overlap() {
            Overlap::Previous(s) => (s, 0.0),
            Overlap::Next(s) => (0.0, s),
            Overlap::PreviousAndNext { previous, next } => (previous, next),
            Overlap::None => (0.0, 0.0),
        };

        for (side, overlap_secs) in [("previous", previous), ("next", next)] {
            if overlap_secs > scene_secs {
                return Err(FFramesError::Overflow(
                    format!(
                        "Scene '{}' {side} overlap ({overlap_secs}s) exceeds scene duration ({scene_secs}s)",
                        scene.name()
                    ),
                    ((overlap_secs - scene_secs) * 1000.0) as usize,
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FFramesContext, Frame, Scene, Svgr};

    #[derive(Debug)]
    struct TestScene {
        seconds: f32,
        previous: f32,
        next: f32,
    }

    impl Scene for TestScene {
        fn duration(&self) -> Duration<'_> {
            Duration::Seconds(self.seconds)
        }

        fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
            Svgr::empty()
        }

        fn overlap(&self) -> Overlap {
            Overlap::PreviousAndNext { previous: self.previous, next: self.next }
        }
    }

    #[test]
    fn auto_duration_without_scenes_or_media_is_rejected() {
        let result = validate_video(&Duration::Auto, &Scenes::empty(), None);
        assert!(matches!(result, Err(FFramesError::MissingDurationOrScenes)));
    }

    #[test]
    fn missing_duration_audio_is_rejected() {
        let result = validate_video(&Duration::FromAudio("music.mp3"), &Scenes::empty(), None);
        assert!(matches!(result, Err(FFramesError::RequiredAudioNotFound(name)) if name == "music.mp3"));
    }

    #[test]
    fn overlap_longer_than_scene_is_rejected() {
        let scene = TestScene { seconds: 1.0, previous: 0.0, next: 1.5 };
        let scenes = Scenes::from(vec![&scene as &dyn Scene]);
        let result = validate_video(&Duration::Auto, &scenes, None);
        assert!(matches!(result, Err(FFramesError::Overflow(_, 500))));
    }

    #[test]
    fn valid_scenes_pass() {
        let first = TestScene { seconds: 2.0, previous: 0.0, next: 0.5 };
        let second = TestScene { seconds: 2.0, previous: 0.5, next: 0.0 };
        let scenes = Scenes::from(vec![&first as &dyn Scene, &second]);
        assert!(validate_video(&Duration::Auto, &scenes, None).is_ok());
    }
}
