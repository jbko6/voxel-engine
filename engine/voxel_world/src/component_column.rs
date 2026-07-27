use std::{any::Any};
use indexmap::IndexMap;

/// A column of components in the entity table.
pub struct ComponentColumn<T> {
    counter: u32,
    data: IndexMap<u32, T>,
}

impl<T> ComponentColumn<T> {
    /// Create a new component column
    pub fn new() -> Self {
        Self {
            counter: 0,
            data: IndexMap::new(),
        }
    }

    /// Add a new component to the column with the specified value, returning the index of the new component
    pub fn add(&mut self, value: T) -> u32 {
        self.data.insert(self.counter, value);
        self.counter += 1;
        self.counter - 1
    }

    /// Update a component in the column, returning an error if the component does not exist
    pub fn update(&mut self, index: &u32, value: T) -> Result<(), String> {
        if let Some(component) = self.data.get_mut(index) {
            *component = value;
            Ok(())
        } else {
            Err(format!("Component at index {} does not exist", index))
        }
    }

    /// Remove a component from the column, returning the removed component if it existed
    pub fn remove(&mut self, index: &u32) -> Option<T> {
        self.counter -= 1;
        self.data.swap_remove(index)
    }

    /// Get a reference to a component in the column by its row index
    pub fn get(&self, index: &u32) -> Option<&T> {
        self.data.get(index)
    }

    /// Get an iterator over all components in the column
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.values()
    }

    /// Get a mutable iterator over all components in the column
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.data.values_mut()
    }
}

/// A type-erased column of components in the entity table.
pub trait AnyColumn {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    // fn add(&mut self) -> u32;
    fn remove(&mut self, index: &u32);
    fn clone_func(&self) -> ColumnFactory;
    fn move_data_to(&mut self, other: &mut dyn AnyColumn, src_index: &u32);
}

impl<T: 'static> AnyColumn for ComponentColumn<T> {
    /// Get a reference to the column as a type-erased Any
    fn as_any(&self) -> &dyn Any {
        self
    }

    /// Get a mutable reference to the column as a type-erased Any
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    // /// Add a new component to the column with a default value, returning the index of the new component
    // fn add(&mut self) -> u32 {
    //     self.data.insert(self.counter, T::default());
    //     self.counter += 1;
    //     self.counter - 1
    // }

    /// Remove a component from the column by its index
    fn remove(&mut self, index: &u32) {
        self.counter -= 1;
        self.data.swap_remove(index);
    }

    /// Get a function that can create a new column of the same type
    fn clone_func(&self) -> ColumnFactory {
        make_column::<T>
    }

    /// Move a component from this column to another column of the same type
    /// This removes the component from this column and updates the other column with the moved component
    fn move_data_to(&mut self, other: &mut dyn AnyColumn, src_index: &u32) {
        if let Some(other_column) = other.as_any_mut().downcast_mut::<ComponentColumn<T>>() {
            if let Some(value) = self.remove(src_index) {
                other_column.add(value);
            }
        } else {
            panic!("Type mismatch when moving component to another column");
        }
    }
}

pub type ColumnFactory = fn() -> Box<dyn AnyColumn>;

pub fn make_column<T: 'static>() -> Box<dyn AnyColumn> {
    Box::new(ComponentColumn::<T>::new())
}