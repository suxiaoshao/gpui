pub(super) fn single_selected_index(current_index: usize, checkeds: &[bool]) -> usize {
    checkeds
        .iter()
        .enumerate()
        .find_map(|(index, checked)| (*checked && index != current_index).then_some(index))
        .unwrap_or(current_index)
}

#[cfg(test)]
mod tests {
    use super::single_selected_index;

    #[test]
    fn single_select_switches_to_new_checked_item() {
        assert_eq!(single_selected_index(0, &[true, false, true]), 2);
        assert_eq!(single_selected_index(2, &[false, true, true]), 1);
    }

    #[test]
    fn single_select_keeps_current_when_current_is_clicked_off() {
        assert_eq!(single_selected_index(1, &[false, false, false]), 1);
    }

    #[test]
    fn single_select_keeps_current_when_no_new_item_is_checked() {
        assert_eq!(single_selected_index(2, &[false, false, true]), 2);
        assert_eq!(single_selected_index(2, &[]), 2);
    }
}
