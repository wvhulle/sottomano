UPDATE pinned_group_items
SET item_key = 'Charge'
WHERE group_family = 'power'
    AND item_key = 'Battery';

UPDATE pinned_group_items
SET item_key = 'Power flow'
WHERE group_family = 'power'
    AND item_key = 'Power draw';

UPDATE pinned_group_items
SET item_key = 'Time left'
WHERE group_family = 'power'
    AND item_key = 'Battery time';
