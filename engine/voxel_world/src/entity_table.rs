use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use sorted_vec::SortedVec;

use crate::{AnyColumn, ColumnFactory};
use crate::component_column::ComponentColumn;

type RowIndex = u32;

/// A table for storing entities and their components.
pub struct EntityTable {
    counter: RowIndex,
    hash: u64,
    /// A map of component names to their corresponding columns.
    components: HashMap<String, Box<dyn AnyColumn>>,
}

impl EntityTable {
    /// Create an empty entity table with no components
    pub fn empty() -> Self {
        Self {
            counter: RowIndex::MAX,
            hash: u64::MAX,
            components: HashMap::new(),
        }
    }

    /// Derive a new entity table from an existing one
    /// The returned entity table will have all the components of the original table,
    /// plus the component passed as a parameter
    pub fn with(&self, component: (String, ColumnFactory)) -> Self {
        self.with_many(&vec![component])
    }

    pub fn with_many(&self, new_components: &Vec<(String, ColumnFactory)>) -> Self {
        // Hash
        let mut component_names = SortedVec::from_iter(self.components.keys());
        component_names.extend(new_components.iter().map(|(name, _)| name));
        let mut h = DefaultHasher::new();
        for name in component_names {
            name.hash(&mut h);
        }
        let hash = h.finish();

        // Create component columns
        let mut components = self.components
            .iter()
            .map(|(name, column)| 
                (name.clone(), column.clone_func()()))
            .collect::<HashMap<String, Box<dyn AnyColumn>>>();
        for (name, factory) in new_components {
            components.insert(name.clone(), factory());
        }

        Self {
            counter: 0,
            hash,
            components,
        }
    }

    /// Insert a new component for an entity at the given row index
    /// Should only be called when the entity does not already have a component of this type
    pub fn insert<T: 'static>(&mut self, component_name: &str, component_data: T) {
        if let Some(column) = self.get_column_mut::<T>(component_name) {
            column.add(component_data);
        } else {
            panic!("Component {} not found in entity table", component_name);
        }
    }

    // Update a component for an entity at the given row index
    pub fn update<T: 'static>(&mut self, row_index: RowIndex, component_name: &str, component_data: T) {
        if let Some(column) = self.get_column_mut(component_name) {
            column.update(&row_index, component_data).expect("Failed to update component");
        } else {
            panic!("Component {} not found in entity table", component_name);
        }
    }

    // TODO: bulk updates and inserts of components

    /// Get the value of a component for an entity at the given row index
    /// If possible, iterating over all such components is more efficient
    pub fn get<T: 'static>(&self, row_index: RowIndex, component_name: &str) -> Option<&T> {
        if let Some(column) = self.get_column::<T>(component_name) {
            column.get(&row_index)
        } else {
            None
        }
    }

    /// Moves all component data for some row from this table to another table.
    /// Removes this row when done.
    /// Returns dest index in other table.
    pub fn transfer(&mut self, other: &mut EntityTable, src_index: RowIndex) -> RowIndex {
        for (name, column) in &mut self.components {
            if let Some(other_column) = other.components.get_mut(name) {
                column.move_data_to(other_column.as_mut(), &src_index);
            }
        }
        self.counter -= 1;
        let dest_index = other.counter;
        other.counter += 1;
        dest_index
    }

    /// Remove an entity from the table by its row index
    /// Also removes all component data associated with it
    pub fn remove(&mut self, row_index: RowIndex) {
        for column in self.components.values_mut() {
            column.remove(&row_index);
        }
        self.counter -= 1;
    }

    /// Iterate over the components of a specific type in the table
    pub fn iter_column<T: 'static>(&self, component_name: &str) -> impl Iterator<Item = &T> {
        self.get_column::<T>(component_name).unwrap().iter()
    }

    pub fn iter_columns<T1: 'static, T2: 'static>(&self, component_name1: &str, component_name2: &str) -> impl Iterator<Item = (&T1, &T2)> {
        let column1 = self.get_column::<T1>(component_name1).unwrap();
        let column2 = self.get_column::<T2>(component_name2).unwrap();
        column1.iter().zip(column2.iter())
    }

    /// Iterate over the mutable components of a specific type in the table
    pub fn iter_column_mut<T: 'static>(&mut self, component_name: &str) -> impl Iterator<Item = &mut T> {
        self.get_column_mut::<T>(component_name).unwrap().iter_mut()
    }

    pub fn iter_columns_mut<T1: 'static, T2: 'static>(&mut self, component_name1: &str, component_name2: &str) -> Result<impl Iterator<Item = (&mut T1, &mut T2)>, ()> {
        if let [Some(column1), Some(column2)] = self.components.get_disjoint_mut([component_name1, component_name2]) {
            let column1 = column1.as_any_mut().downcast_mut::<ComponentColumn<T1>>().unwrap();
            let column2 = column2.as_any_mut().downcast_mut::<ComponentColumn<T2>>().unwrap();
            Ok(column1.iter_mut().zip(column2.iter_mut()))
        } else {
            Err(())
        }
    }

    /// Get the hash of the entity table, which is based on the components it contains
    pub fn hash(&self) -> u64 {
        self.hash
    }

    /// Get the names of the components in the entity table
    pub fn component_names(&self) -> SortedVec<String> {
        SortedVec::from_iter(self.components.keys().cloned())
    }

    /// Helper to get a reference to a component column by name
    fn get_column<T: 'static>(&self, component_name: &str) -> Option<&ComponentColumn<T>> {
        self.components.get(component_name)?.as_any().downcast_ref::<ComponentColumn<T>>()
    }

    /// Helper to get a mutable reference to a component column by name
    fn get_column_mut<T: 'static>(&mut self, component_name: &str) -> Option<&mut ComponentColumn<T>> {
        self.components.get_mut(component_name)?.as_any_mut().downcast_mut::<ComponentColumn<T>>()
    }

}