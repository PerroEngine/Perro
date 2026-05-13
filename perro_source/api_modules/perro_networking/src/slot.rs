use super::*;

pub(crate) fn insert_slot<T>(slots: &mut Vec<Option<T>>, value: T) -> u32 {
    if let Some(i) = slots.iter().position(Option::is_none) {
        slots[i] = Some(value);
        return i as u32;
    }
    slots.push(Some(value));
    (slots.len() - 1) as u32
}

pub(crate) fn remove_slot<T>(slots: &mut [Option<T>], id: u32) -> bool {
    let Some(slot) = slots.get_mut(id as usize) else {
        return false;
    };
    slot.take().is_some()
}

pub(crate) fn get_slot<'a, T>(slots: &'a [Option<T>], id: u32, label: &str) -> NetResult<&'a T> {
    slots
        .get(id as usize)
        .and_then(Option::as_ref)
        .ok_or_else(|| NetError::new(NetErrorKind::MissingHandle, format!("missing {label} {id}")))
}

pub(crate) fn get_slot_mut<'a, T>(
    slots: &'a mut [Option<T>],
    id: u32,
    label: &str,
) -> NetResult<&'a mut T> {
    slots
        .get_mut(id as usize)
        .and_then(Option::as_mut)
        .ok_or_else(|| NetError::new(NetErrorKind::MissingHandle, format!("missing {label} {id}")))
}
