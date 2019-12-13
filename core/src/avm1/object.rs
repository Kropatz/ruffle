//! Object trait to expose objects to AVM

use crate::avm1::function::Executable;
use crate::avm1::property::Attribute;
use crate::avm1::return_value::ReturnValue;
use crate::avm1::{Avm1, Error, ScriptObject, StageObject, UpdateContext, Value};
use crate::display_object::DisplayObject;
use enumset::EnumSet;
use gc_arena::{Collect, MutationContext};
use ruffle_macros::enum_trait_object;
use std::collections::HashSet;
use std::fmt::Debug;

/// Represents an object that can be directly interacted with by the AVM
/// runtime.
#[enum_trait_object(
    #[derive(Clone, Collect, Debug, Copy)]
    #[collect(no_drop)]
    pub enum Object<'gc> {
        ScriptObject(ScriptObject<'gc>),
        StageObject(StageObject<'gc>),
    }
)]
pub trait TObject<'gc>: 'gc + Collect + Debug + Into<Object<'gc>> + Clone + Copy {
    /// Retrieve a named property from this object exclusively.
    ///
    /// This function takes a redundant `this` parameter which should be
    /// the object's own `GcCell`, so that it can pass it to user-defined
    /// overrides that may need to interact with the underlying object.
    ///
    /// This function should not inspect prototype chains. Instead, use `get`
    /// to do ordinary property look-up and resolution.
    fn get_local(
        &self,
        name: &str,
        avm: &mut Avm1<'gc>,
        context: &mut UpdateContext<'_, 'gc, '_>,
        this: Object<'gc>,
    ) -> Result<ReturnValue<'gc>, Error>;

    /// Retrieve a named property from the object, or it's prototype.
    fn get(
        &self,
        name: &str,
        avm: &mut Avm1<'gc>,
        context: &mut UpdateContext<'_, 'gc, '_>,
    ) -> Result<ReturnValue<'gc>, Error> {
        if self.has_own_property(name) {
            self.get_local(name, avm, context, (*self).into())
        } else {
            search_prototype(self.proto(), name, avm, context, (*self).into())
        }
    }

    /// Set a named property on this object, or it's prototype.
    fn set(
        &self,
        name: &str,
        value: Value<'gc>,
        avm: &mut Avm1<'gc>,
        context: &mut UpdateContext<'_, 'gc, '_>,
    ) -> Result<(), Error>;

    /// Call the underlying object.
    ///
    /// This function takes a  `this` parameter which generally
    /// refers to the object which has this property, although
    /// it can be changed by `Function.apply`/`Function.call`.
    fn call(
        &self,
        avm: &mut Avm1<'gc>,
        context: &mut UpdateContext<'_, 'gc, '_>,
        this: Object<'gc>,
        args: &[Value<'gc>],
    ) -> Result<ReturnValue<'gc>, Error>;

    /// Construct a host object of some kind and return it's cell.
    ///
    /// As the first step in object construction, the `new` method is called on
    /// the prototype to initialize an object. The prototype may construct any
    /// object implementation it wants, with itself as the new object's proto.
    /// Then, the constructor is `call`ed with the new object as `this` to
    /// initialize the object.
    ///
    /// The arguments passed to the constructor are provided here; however, all
    /// object construction should happen in `call`, not `new`. `new` exists
    /// purely so that host objects can be constructed by the VM.
    fn new(
        &self,
        avm: &mut Avm1<'gc>,
        context: &mut UpdateContext<'_, 'gc, '_>,
        this: Object<'gc>,
        args: &[Value<'gc>],
    ) -> Result<Object<'gc>, Error>;

    /// Delete a named property from the object.
    ///
    /// Returns false if the property cannot be deleted.
    fn delete(&self, gc_context: MutationContext<'gc, '_>, name: &str) -> bool;

    /// Retrieve the `__proto__` of a given object.
    ///
    /// The proto is another object used to resolve methods across a class of
    /// multiple objects. It should also be accessible as `__proto__` from
    /// `get`.
    fn proto(&self) -> Option<Object<'gc>>;

    /// Define a value on an object.
    ///
    /// Unlike setting a value, this function is intended to replace any
    /// existing virtual or built-in properties already installed on a given
    /// object. As such, this should not run any setters; the resulting name
    /// slot should either be completely replaced with the value or completely
    /// untouched.
    ///
    /// It is not guaranteed that all objects accept value definitions,
    /// especially if a property name conflicts with a built-in property, such
    /// as `__proto__`.
    fn define_value(
        &self,
        gc_context: MutationContext<'gc, '_>,
        name: &str,
        value: Value<'gc>,
        attributes: EnumSet<Attribute>,
    );

    /// Define a virtual property onto a given object.
    ///
    /// A virtual property is a set of get/set functions that are called when a
    /// given named property is retrieved or stored on an object. These
    /// functions are then responsible for providing or accepting the value
    /// that is given to or taken from the AVM.
    ///
    /// It is not guaranteed that all objects accept virtual properties,
    /// especially if a property name conflicts with a built-in property, such
    /// as `__proto__`.
    fn add_property(
        &self,
        gc_context: MutationContext<'gc, '_>,
        name: &str,
        get: Executable<'gc>,
        set: Option<Executable<'gc>>,
        attributes: EnumSet<Attribute>,
    );

    /// Checks if the object has a given named property.
    fn has_property(&self, name: &str) -> bool;

    /// Checks if the object has a given named property on itself (and not,
    /// say, the object's prototype or superclass)
    fn has_own_property(&self, name: &str) -> bool;

    /// Checks if a named property can be overwritten.
    fn is_property_overwritable(&self, name: &str) -> bool;

    /// Checks if a named property appears when enumerating the object.
    fn is_property_enumerable(&self, name: &str) -> bool;

    /// Enumerate the object.
    fn get_keys(&self) -> HashSet<String>;

    /// Coerce the object into a string.
    fn as_string(&self) -> String;

    /// Get the object's type string.
    fn type_of(&self) -> &'static str;

    /// Get the underlying script object, if it exists.
    fn as_script_object(&self) -> Option<ScriptObject<'gc>>;

    /// Get the underlying display node for this object, if it exists.
    fn as_display_object(&self) -> Option<DisplayObject<'gc>>;

    /// Get the underlying executable for this object, if it exists.
    fn as_executable(&self) -> Option<Executable<'gc>>;

    fn as_ptr(&self) -> *const ObjectPtr;

    /// Check if this object is in the prototype chain of the specified test object.
    fn is_prototype_of(&self, other: Object<'gc>) -> bool {
        let mut proto = other.proto();

        while let Some(proto_ob) = proto {
            if self.as_ptr() == proto_ob.as_ptr() {
                return true;
            }

            proto = proto_ob.proto();
        }

        false
    }

    /// Get the length of this object, as if it were an array.
    fn get_length(&self) -> usize;

    /// Gets a copy of the array storage behind this object.
    fn get_array(&self) -> Vec<Value<'gc>>;

    /// Sets the length of this object, as if it were an array.
    ///
    /// Increasing this value will fill the gap with Value::Undefined.
    /// Decreasing this value will remove affected items from both the array and properties storage.
    fn set_length(&self, gc_context: MutationContext<'gc, '_>, length: usize);

    /// Gets a property of this object as if it were an array.
    ///
    /// Array element lookups do not respect the prototype chain, and will ignore virtual properties.
    fn get_array_element(&self, index: usize) -> Value<'gc>;

    /// Sets a property of this object as if it were an array.
    ///
    /// This will increase the "length" of this object to encompass the index, and return the new length.
    /// Any gap created by increasing the length will be filled with Value::Undefined, both in array
    /// and property storage.
    fn set_array_element(
        &self,
        index: usize,
        value: Value<'gc>,
        gc_context: MutationContext<'gc, '_>,
    ) -> usize;

    /// Deletes a property of this object as if it were an array.
    ///
    /// This will not rearrange the array or adjust the length, nor will it affect the properties
    /// storage.
    fn delete_array_element(&self, index: usize, gc_context: MutationContext<'gc, '_>);
}

pub enum ObjectPtr {}

impl<'gc> Object<'gc> {
    pub fn ptr_eq(a: Object<'gc>, b: Object<'gc>) -> bool {
        a.as_ptr() == b.as_ptr()
    }
}

pub fn search_prototype<'gc>(
    mut proto: Option<Object<'gc>>,
    name: &str,
    avm: &mut Avm1<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
    this: Object<'gc>,
) -> Result<ReturnValue<'gc>, Error> {
    let mut depth = 0;

    while proto.is_some() {
        if depth == 255 {
            return Err("Encountered an excessively deep prototype chain.".into());
        }

        if proto.unwrap().has_own_property(name) {
            return proto.unwrap().get_local(name, avm, context, this);
        }

        proto = proto.unwrap().proto();
        depth += 1;
    }

    Ok(Value::Undefined.into())
}
