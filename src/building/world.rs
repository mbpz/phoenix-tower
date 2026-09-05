//! 完整建筑数据与有界编辑历史。实体的生成/销毁由调用方负责。

use bevy::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

/// PRD §3.2：保留最近 20 次放置的撤销能力，不限制建筑规模。
const MAX_HISTORY: usize = 20;

#[derive(Resource, Default)]
pub struct PlacedBlocks {
    /// 完整建筑，按放置顺序排列；不受撤销历史上限影响。
    pub records: Vec<PlacedRecord>,
    /// 已撤销记录：原实体已销毁，重做时由调用方创建新实体。
    pub redo: Vec<PlacedRecord>,
    pub occupied: HashSet<IVec3>,
    /// (x, z) → 最高占用格 y + 1。
    pub col_top: HashMap<(i32, i32), i32>,
    /// 每次成功编辑、整体替换或实体复原后递增，供派生系统门控。
    pub revision: u64,
    /// 只保存最近的编辑引用，淘汰时绝不删除建筑记录或实体。
    undo: VecDeque<Entity>,
}

pub struct PlacedRecord {
    pub entity: Entity,
    pub def_id: String,
    pub anchor: IVec3,
    /// 90° × rot。
    pub rot: u8,
    pub cells: Vec<IVec3>,
}

impl PlacedBlocks {
    /// 新放置建立新的编辑分支，清空已失效的重做记录。
    pub fn place(&mut self, record: PlacedRecord) {
        self.redo.clear();
        self.remember(record.entity);
        self.insert(record);
        self.revision += 1;
    }

    /// 撤销最近的可撤销放置；返回待销毁实体及配额退还所需的数据。
    pub fn undo(&mut self) -> Option<&PlacedRecord> {
        let entity = *self.undo.back()?;
        let index = self
            .records
            .iter()
            .position(|record| record.entity == entity)?;
        self.undo.pop_back();
        let record = self.records.remove(index);
        self.rebuild_indexes();
        self.redo.push(record);
        self.revision += 1;
        self.redo.last()
    }

    /// 调用方校验挑战配额、生成实体后提交重做；保留剩余重做栈。
    pub fn redo(&mut self, entity: Entity) -> bool {
        let Some(mut record) = self.redo.pop() else {
            return false;
        };
        record.entity = entity;
        self.remember(entity);
        self.insert(record);
        self.revision += 1;
        true
    }

    /// 拆除按查询找到的记录；实体销毁及其他玩法副作用由调用方处理。
    pub fn remove(&mut self, index: usize) -> PlacedRecord {
        let record = self.records.remove(index);
        self.undo.retain(|entity| *entity != record.entity);
        self.redo.clear();
        self.rebuild_indexes();
        self.revision += 1;
        record
    }

    /// 批量导入开启新编辑会话；存档不恢复历史或旧实体引用。
    pub fn replace_all(&mut self, records: Vec<PlacedRecord>) {
        self.records = records;
        self.undo.clear();
        self.redo.clear();
        self.rebuild_indexes();
        self.revision += 1;
    }

    pub fn clear(&mut self) {
        self.replace_all(Vec::new());
    }

    /// 重力测试复原：只替换实体引用，保持数据、编辑顺序与重做内容。
    pub fn rebuild_entities(&mut self, mut spawn: impl FnMut(&PlacedRecord) -> Entity) {
        for record in &mut self.records {
            let entity = spawn(record);
            for undo_entity in &mut self.undo {
                if *undo_entity == record.entity {
                    *undo_entity = entity;
                }
            }
            record.entity = entity;
        }
        self.revision += 1;
    }

    fn remember(&mut self, entity: Entity) {
        self.undo.push_back(entity);
        if self.undo.len() > MAX_HISTORY {
            self.undo.pop_front();
        }
    }

    fn insert(&mut self, record: PlacedRecord) {
        for cell in &record.cells {
            self.occupied.insert(*cell);
            let top = self.col_top.entry((cell.x, cell.z)).or_insert(0);
            *top = (*top).max(cell.y + 1);
        }
        self.records.push(record);
    }

    fn rebuild_indexes(&mut self) {
        self.occupied.clear();
        self.col_top.clear();
        // 存档允许重叠 footprint：从存活记录重建并集，不能直接释放被删块的格子。
        for record in &self.records {
            for cell in &record.cells {
                self.occupied.insert(*cell);
                let top = self.col_top.entry((cell.x, cell.z)).or_insert(0);
                *top = (*top).max(cell.y + 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(world: &mut World, x: i32, y: i32) -> PlacedRecord {
        let anchor = IVec3::new(x, y, 0);
        PlacedRecord {
            entity: world.spawn_empty().id(),
            def_id: "test".into(),
            anchor,
            rot: 0,
            cells: vec![anchor],
        }
    }

    fn assert_indexes(stack: &PlacedBlocks) {
        let occupied: HashSet<_> = stack
            .records
            .iter()
            .flat_map(|r| r.cells.iter().copied())
            .collect();
        assert_eq!(stack.occupied, occupied);
        let mut tops: HashMap<(i32, i32), i32> = HashMap::new();
        for cell in occupied {
            tops.entry((cell.x, cell.z))
                .and_modify(|top| *top = (*top).max(cell.y + 1))
                .or_insert((cell.y + 1).max(0));
        }
        assert_eq!(stack.col_top, tops);
    }

    #[test]
    fn history_limit_never_limits_building_size() {
        let mut world = World::new();
        let mut stack = PlacedBlocks::default();
        for x in 0..30 {
            stack.place(record(&mut world, x, 0));
        }
        assert_eq!(stack.records.len(), 30);
        assert_eq!(stack.occupied.len(), 30);
        assert_eq!(stack.undo.len(), MAX_HISTORY);
        assert_indexes(&stack);
        for _ in 0..MAX_HISTORY {
            assert!(stack.undo().is_some());
            assert_indexes(&stack);
        }
        assert!(stack.undo().is_none());
        assert_eq!(stack.records.len(), 10);
        assert!(stack.occupied.contains(&IVec3::ZERO));
        for _ in 0..MAX_HISTORY {
            let entity = world.spawn_empty().id();
            assert!(stack.redo(entity));
            assert_indexes(&stack);
        }
        assert_eq!(stack.records.len(), 30);
        assert!(stack.redo.is_empty());
    }

    #[test]
    fn undo_and_redo_preserve_rotation_and_column_heights() {
        let mut world = World::new();
        let mut stack = PlacedBlocks::default();
        stack.place(record(&mut world, 0, 0));
        let mut upper = record(&mut world, 0, 3);
        upper.rot = 1;
        upper.cells = vec![IVec3::new(0, 3, 0), IVec3::new(0, 3, 1)];
        stack.place(upper);
        assert_eq!(stack.col_top[&(0, 0)], 4);
        let removed = stack.undo().unwrap();
        assert_eq!(removed.rot, 1);
        assert_eq!(removed.cells.len(), 2);
        assert_eq!(stack.col_top[&(0, 0)], 1);
        assert!(!stack.col_top.contains_key(&(0, 1)));
        let replacement = world.spawn_empty().id();
        assert!(stack.redo(replacement));
        assert_eq!(stack.records.last().unwrap().entity, replacement);
        assert_eq!(stack.records.last().unwrap().rot, 1);
        assert_indexes(&stack);
    }

    #[test]
    fn new_placement_invalidates_redo() {
        let mut world = World::new();
        let mut stack = PlacedBlocks::default();
        stack.place(record(&mut world, 0, 0));
        stack.undo();
        stack.place(record(&mut world, 1, 0));
        assert!(stack.redo.is_empty());
        assert!(!stack.redo(Entity::PLACEHOLDER));
        assert_indexes(&stack);
    }

    #[test]
    fn demolition_removes_history_entry_and_invalidates_redo() {
        let mut world = World::new();
        let mut stack = PlacedBlocks::default();
        for y in 0..3 {
            stack.place(record(&mut world, 0, y));
        }
        stack.undo();
        let removed = stack.remove(0);
        assert_eq!(removed.anchor.y, 0);
        assert!(stack.redo.is_empty());
        assert_eq!(stack.undo().unwrap().anchor.y, 1);
        assert!(stack.undo().is_none());
        assert_indexes(&stack);
    }

    #[test]
    fn replacement_and_clear_reset_both_histories() {
        let mut world = World::new();
        let mut stack = PlacedBlocks::default();
        stack.place(record(&mut world, 0, 0));
        stack.place(record(&mut world, 1, 0));
        stack.undo();
        let revision = stack.revision;
        stack.replace_all(vec![record(&mut world, 2, 5)]);
        assert_eq!(stack.revision, revision + 1);
        assert!(stack.redo.is_empty());
        assert!(stack.undo().is_none());
        assert_eq!(stack.col_top[&(2, 0)], 6);
        assert_indexes(&stack);
        stack.clear();
        assert!(stack.records.is_empty());
        assert_indexes(&stack);
    }

    #[test]
    fn gravity_entity_rebuild_keeps_undo_references_current() {
        let mut world = World::new();
        let mut stack = PlacedBlocks::default();
        stack.place(record(&mut world, 0, 0));
        stack.place(record(&mut world, 1, 0));
        stack.undo();
        let new_entity = world.spawn_empty().id();
        let revision = stack.revision;
        stack.rebuild_entities(|_| new_entity);
        assert_eq!(stack.revision, revision + 1);
        let redo_entity = world.spawn_empty().id();
        assert!(stack.redo(redo_entity));
        assert_eq!(stack.undo().unwrap().entity, redo_entity);
        assert_eq!(stack.undo().unwrap().entity, new_entity);
        assert_eq!(stack.redo.len(), 2);
        assert_indexes(&stack);
    }

    #[test]
    fn removing_overlapping_import_keeps_surviving_cells_occupied() {
        let mut world = World::new();
        let mut stack = PlacedBlocks::default();
        stack.replace_all(vec![record(&mut world, 0, 0), record(&mut world, 0, 0)]);
        stack.remove(0);
        assert_eq!(stack.records.len(), 1);
        assert!(stack.occupied.contains(&IVec3::ZERO));
        assert_eq!(stack.col_top[&(0, 0)], 1);
        assert_indexes(&stack);
    }

    #[test]
    fn no_op_history_does_not_change_revision() {
        let mut stack = PlacedBlocks::default();
        assert!(stack.undo().is_none());
        assert!(!stack.redo(Entity::PLACEHOLDER));
        assert_eq!(stack.revision, 0);
    }

    /// Data-path benchmark, not a GPU or physics benchmark. Timings are evidence,
    /// not CI thresholds, so slow runners do not produce false correctness failures.
    #[test]
    #[ignore = "opt-in 50k editable-world benchmark; run with --ignored --nocapture"]
    fn editable_world_50k_history_and_save_roundtrip() {
        use std::time::Instant;
        let mut world = World::new();
        let mut stack = PlacedBlocks::default();
        let start = Instant::now();
        for index in 0..50_000 {
            let anchor = IVec3::new(index % 250, 0, index / 250);
            stack.place(PlacedRecord {
                entity: world.spawn_empty().id(),
                def_id: "hongzhu4".into(),
                anchor,
                rot: 0,
                cells: (0..4).map(|y| anchor + IVec3::Y * y).collect(),
            });
        }
        let place_ms = start.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(stack.records.len(), 50_000);
        assert_eq!(stack.occupied.len(), 200_000);
        assert_indexes(&stack);
        let start = Instant::now();
        for _ in 0..MAX_HISTORY {
            let entity = stack.undo().unwrap().entity;
            assert!(world.despawn(entity));
        }
        let undo_ms = start.elapsed().as_secs_f64() * 1000.0;
        assert!(stack.undo().is_none());
        assert_eq!(stack.records.len(), 49_980);
        let start = Instant::now();
        for _ in 0..MAX_HISTORY {
            assert!(stack.redo(world.spawn_empty().id()));
        }
        let redo_ms = start.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(stack.records.len(), 50_000);
        assert_indexes(&stack);
        let start = Instant::now();
        let blueprint = crate::building::blueprint::load_blueprint();
        let save = crate::save::build_save(&stack, &blueprint, "free");
        let bytes = crate::save::encode_bincode(&save).unwrap();
        let restored = crate::save::decode_bincode(&bytes).unwrap();
        let save_ms = start.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(restored.blocks.len(), 50_000);
        assert_eq!(restored.meta.block_count, 50_000);
        for (stored, record) in restored.blocks.iter().zip(&stack.records) {
            assert_eq!(stored.id, record.def_id);
            assert_eq!(
                stored.cell,
                (record.anchor.x, record.anchor.y, record.anchor.z)
            );
            assert_eq!(stored.rot_90, record.rot);
            assert!(world.get_entity(record.entity).is_ok());
        }
        println!("EDITABLE_WORLD_BENCH blocks=50000 cells=200000 place_ms={place_ms:.2} undo20_ms={undo_ms:.2} redo20_ms={redo_ms:.2} save_roundtrip_ms={save_ms:.2} bytes={}", bytes.len());
    }
}
