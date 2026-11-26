//! Native compositor animations: no per-frame app timer or repeated icon rasterization.
use crate::model::{Activity, AttentionStyle, Item};
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSStatusBarButton, NSWorkspace};
use objc2_foundation::{NSArray, NSNumber, NSString};
use objc2_quartz_core::{CAKeyframeAnimation, CAMediaTiming, kCAAnimationDiscrete};
fn key() -> Retained<NSString> {
    NSString::from_str("badgeify-unread-attention")
}
pub fn clear(button: &NSStatusBarButton) {
    if let Some(layer) = button.layer() {
        layer.removeAnimationForKey(&key());
    }
}
fn sequence(style: AttentionStyle) -> Option<(&'static [f64], &'static [f64], f64)> {
    match style {
        AttentionStyle::None => None,
        AttentionStyle::Pulse => Some((&[1.0, 0.35, 1.0], &[0.0, 0.5, 1.0], 2.0)),
        AttentionStyle::Blink => Some((&[1.0, 0.15, 1.0], &[0.0, 0.5, 1.0], 1.4)),
        AttentionStyle::DoubleBlink => Some((
            &[1.0, 0.15, 1.0, 0.15, 1.0, 1.0],
            &[0.0, 0.12, 0.24, 0.36, 0.48, 1.0],
            2.4,
        )),
    }
}
pub fn apply(button: &NSStatusBarButton, item: &Item, state: &Activity) {
    clear(button);
    if !item.enabled
        || !item.appearance.notifications
        || !state.has_unread()
        || NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
    {
        return;
    }
    let Some((values, times, duration)) = sequence(item.appearance.attention) else {
        return;
    };
    button.setWantsLayer(true);
    let Some(layer) = button.layer() else {
        return;
    };
    let animation = CAKeyframeAnimation::animationWithKeyPath(Some(&NSString::from_str("opacity")));
    let values: Vec<Retained<NSNumber>> = values
        .iter()
        .map(|v| NSNumber::numberWithDouble(*v))
        .collect();
    let objects: Vec<&AnyObject> = values.iter().map(|v| &**v as &AnyObject).collect();
    unsafe {
        animation.setValues(Some(&NSArray::from_slice(&objects)));
    }
    let times: Vec<_> = times
        .iter()
        .map(|v| NSNumber::numberWithDouble(*v))
        .collect();
    animation.setKeyTimes(Some(&NSArray::from_retained_slice(&times)));
    animation.setDuration(duration);
    animation.setRepeatCount(f32::INFINITY);
    if item.appearance.attention != AttentionStyle::Pulse {
        animation.setCalculationMode(unsafe { kCAAnimationDiscrete });
    }
    layer.addAnimation_forKey(&animation, Some(&key()));
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attention_sequences_are_bounded_and_return_to_full_opacity() {
        assert!(sequence(AttentionStyle::None).is_none());
        for style in [
            AttentionStyle::Pulse,
            AttentionStyle::Blink,
            AttentionStyle::DoubleBlink,
        ] {
            let (values, times, duration) = sequence(style).unwrap();
            assert_eq!(values.len(), times.len());
            assert_eq!(values.last(), Some(&1.0));
            assert_eq!(times.first(), Some(&0.0));
            assert_eq!(times.last(), Some(&1.0));
            assert!(times.windows(2).all(|w| w[0] < w[1]));
            assert!(duration >= 1.0);
        }
    }
}
