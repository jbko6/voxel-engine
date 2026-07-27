use std::collections::HashMap;
use std::fmt::Display;
use sorted_vec::SortedVec;

use crate::{entity_table::EntityTable, make_column};

type EntityId = u64;
type TableId = u64;

/// An entry in the world that maps an entity ID to its corresponding entity table and row index.
struct EntityEntry {
    table_index: TableId,
    row_index: u32
}

/// A world that contains entities and their components.
pub struct World {
    counter: EntityId,
    entity_tables: HashMap<TableId, EntityTable>,
    entities: HashMap<u64, EntityEntry>,
    component_group_to_table: HashMap<SortedVec<String>, TableId>,
}

impl World {
    /// Create a new world with an empty entity table
    pub fn new() -> Self {
        let mut entity_tables = HashMap::new();
        entity_tables.insert(TableId::MAX, EntityTable::empty());
        let mut component_group_to_table = HashMap::new();
        component_group_to_table.insert(SortedVec::new(), TableId::MAX);
        Self {
            counter: 0,
            entity_tables,
            entities: HashMap::new(),
            component_group_to_table,
        }
    }

    /// Spawn a new entity in the world, returning its entity ID
    pub fn spawn(&mut self) -> EntityId {
        let id = self.counter;
        self.counter += 1;

        // Add the entity to the default entity table
        self.entities.insert(id, EntityEntry {
            table_index: TableId::MAX,
            row_index: 0
        });

        id
    }

    pub fn emplace<T: 'static>(&mut self, entity_id: EntityId, component_name: &str, component_data: T) {
        // Find the entity entry
        let entry = self.entities.get_mut(&entity_id).expect("Entity ID not found");

        // Find existing component names for the entity
        let old_table = self.entity_tables.get(&entry.table_index).expect("Entity table not found");
        let mut components: SortedVec<String> = old_table.component_names();
        components.push(component_name.to_string());

        // Get new table for the component group, or create one if it doesn't exist
        let new_table_id = if let Some(&table_id) = self.component_group_to_table.get(&components) {
            table_id
        } else {
            // Create a new entity table for this component group
            let new_table = old_table.with((component_name.to_string(), make_column::<T>));
            self.component_group_to_table.insert(components.clone(), new_table.hash());
            let hash = new_table.hash();
            self.entity_tables.insert(hash, new_table);
            hash
        };

        // Move the entity to the new table
        if let [Some(old_table), Some(new_table)] = self.entity_tables.get_disjoint_mut([&entry.table_index, &new_table_id]) {
            entry.row_index = old_table.transfer(new_table, entry.row_index);
        }
        entry.table_index = new_table_id;

        // Add the new component data to the entity in the new table
        let new_table = self.entity_tables.get_mut(&entry.table_index).expect("Entity table not found");
        new_table.insert(component_name, component_data);
    }

    /// Update the value of a component for an entity
    pub fn update<T: 'static>(&mut self, entity_id: EntityId, component_name: &str, component_data: T) {
        // Find the entity entry
        let entry = self.entities.get_mut(&entity_id).expect("Entity ID not found");

        // Update the component data for the entity in its current table
        let table = self.entity_tables.get_mut(&entry.table_index).expect("Entity table not found");
        table.update(entry.row_index, component_name, component_data);
    }

    /// Iterate over all components of a specific type in the world
    pub fn iter<T: 'static>(&self, component_name: &str) -> impl Iterator<Item = &T> {
        self.component_group_to_table
            .iter()
            .filter(|(components, _)| components.contains(&component_name.to_string()))
            .map(|(_, table_id)| self.entity_tables.get(table_id).expect("Entity table not found"))
            .flat_map(|table| table.iter_column::<T>(component_name))
    }

    pub fn iter_columns<T1: 'static, T2: 'static>(&self, component_name1: &str, component_name2: &str) -> impl Iterator<Item = (&T1, &T2)> {
        self.component_group_to_table
            .iter()
            .filter(|(components, _)| components.contains(&component_name1.to_string()) && components.contains(&component_name2.to_string()))
            .map(|(_, table_id)| self.entity_tables.get(table_id).expect("Entity table not found"))
            .flat_map(move |table| table.iter_columns::<T1, T2>(component_name1, component_name2))
    }

    /// Iterate over all components of a specific type in the world, mutably
    pub fn iter_mut<T: 'static>(&mut self, component_name: &str) -> impl Iterator<Item = &mut T> {
        let ids = self.component_group_to_table
            .iter()
            .filter(|(components, _)| components.contains(&component_name.to_string()))
            .map(|(_, table_id)| *table_id)
            .collect::<Vec<_>>();
        self.entity_tables
            .iter_mut()
            .filter(move |(table_id, _)| ids.contains(table_id))
            .flat_map(move |(_, table)| table.iter_column_mut::<T>(component_name))
    }

    /// Iterate over all columns of a specific type in the world, mutably
    pub fn iter_columns_mut<T1: 'static, T2: 'static>(&mut self, component_name1: &str, component_name2: &str) -> impl Iterator<Item = (&mut T1, &mut T2)> {
        let ids = self.component_group_to_table
            .iter()
            .filter(|(components, _)| components.contains(&component_name1.to_string()) && components.contains(&component_name2.to_string()))
            .map(|(_, table_id)| *table_id)
            .collect::<Vec<_>>();
        self.entity_tables
            .iter_mut()
            .filter(move |(table_id, _)| ids.contains(table_id))
            .flat_map(move |(_, table)| table.iter_columns_mut::<T1, T2>(component_name1, component_name2).unwrap())
    }
}

impl Display for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "World with {} entities and {} entity tables", self.entities.len(), self.entity_tables.len())?;
        Ok(())
    }
}