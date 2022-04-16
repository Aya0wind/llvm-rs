use inkwell::basic_block::BasicBlock;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::values::{ArrayValue, BasicValue, FunctionValue};
use nom::combinator::value;

use crate::ast::nodes::{ArrayInitializer, Ast, BinaryOpExpression, CodeBlock, Expr, ForLoop, FunctionCall, FunctionDefinition, FunctionPrototype, Identifier, IfStatement, NumberLiteral, Position, Statement, StructureInitializer, TypeDeclarator, UnaryOpExpression, Variable, WhileLoop};
use crate::code_generator::builder::{LEArrayValue, LEBasicType, LEBasicTypeEnum, LEBasicValue, LEBasicValueEnum, LEBoolType, LEBoolValue, LEFloatValue, LEFunctionType, LEFunctionValue, LEGenerator, LEIntegerValue, LEPointerValue, LEStructType, LEStructValue, LEType, LEValue, LEVectorValue, Result};
use crate::code_generator::builder::binary_operator_builder::{CompareBinaryOperator, LogicBinaryOperator};
use crate::code_generator::builder::expression::ExpressionValue;
use crate::error::CompileError;
use crate::lexer::{Number, Operator};

pub struct CodeGenerator<'ctx> {
    pub generator: LEGenerator<'ctx>,
    pub current_pos: Position,
}

impl<'ctx> CodeGenerator<'ctx> {
    fn build_expression(&mut self, value: &Expr) -> Result<ExpressionValue<'ctx>> {
        match value {
            Expr::UnaryOperator(n) => { self.build_unary_operator_expression(n) }
            Expr::BinaryOperator(n) => { self.build_binary_operator_expression(n) }
            Expr::NumberLiteral(n) => { self.build_number_literal_expression(n) }
            Expr::CallExpression(n) => { self.build_call_expression(n) }
            Expr::Identifier(n) => { self.build_identifier_expression(n) }
            Expr::ArrayInitializer(n) => { self.build_array_initializer(n) }
            Expr::StructureInitializer(n) => { self.build_structure_initializer(n) }
            _ => { unimplemented!() }
            // Expr::StringLiteral(n) => { Ok(Some(self.build_string_literal(n)?)) }
        }
    }


    fn build_structure_initializer(&mut self, expr: &StructureInitializer) -> Result<ExpressionValue<'ctx>> {
        let struct_type = self.generator.get_generic_type(&TypeDeclarator::TypeIdentifier(expr.structure_name.clone()))?;
        if let LEBasicTypeEnum::Struct(struct_type) = struct_type {
            let initializer_member_num = expr.member_initial_values.len();
            if struct_type.get_llvm_type().get_field_types().len() != initializer_member_num {
                return Err(CompileError::TypeMismatched { expect: struct_type.to_string(), found: expr.structure_name.clone() });
            }
            let mut value_array = vec![];
            for (name, initial_value) in expr.member_initial_values.iter() {
                let value = self.build_expression(initial_value.as_ref())?;
                value_array.push((struct_type.get_member_offset(name).unwrap(), value));
            }
            value_array.sort_unstable_by(|x, y| x.0.cmp(&y.0));
            let struct_llvm_value = &value_array.into_iter().map(|x| self.generator.read_expression_value(x.1).unwrap().to_llvm_basic_value_enum()).collect::<Vec<_>>();
            let struct_value = struct_type.get_llvm_type().const_named_struct(&struct_llvm_value);
            Ok(ExpressionValue::Right(LEStructValue { ty: struct_type, llvm_value: struct_value }.to_le_value_enum()))
        } else {
            Err(CompileError::TypeMismatched { expect: "Struct".into(), found: struct_type.name().into() })
        }
    }

    fn build_unary_operator_expression(&mut self, expr: &UnaryOpExpression) -> Result<ExpressionValue<'ctx>> {
        let value = self.build_expression(expr.expr.as_ref())?;
        match expr.op {
            Operator::Plus => {
                Ok(value)
            }
            Operator::Sub => {
                Ok(ExpressionValue::Right(self.generator.build_neg(value)?))
            }
            // Operator::Not => {
            //
            // }
            // Operator::Rev => {}
            _ => { unimplemented!() }
        }
    }


    fn build_array_initializer(&mut self, value: &ArrayInitializer) -> Result<ExpressionValue<'ctx>> {
        if value.elements.is_empty() {
            Err(CompileError::NotAllowZeroLengthArray)
        } else {
            let mut array_values = vec![];
            for v in value.elements.iter() {
                let expr = self.build_expression(v)?;
                array_values.push(self.generator.read_expression_value(expr)?);
            }
            let first_value = array_values.first().unwrap();
            let element_type = LEBasicValue::get_le_type(first_value);
            let array_type = LEBasicType::get_array_type(&element_type, value.elements.len() as u32);
            for others in array_values.iter().skip(1) {
                if others.get_le_type() != element_type {
                    return Err(CompileError::TypeMismatched { expect: first_value.get_le_type().to_string(), found: others.get_le_type().to_string() });
                }
            }

            match element_type {
                LEBasicTypeEnum::Integer(t) => {
                    let array_initial_values = array_values.into_iter().map(|v| v.try_into().unwrap()).collect::<Vec<LEIntegerValue>>();
                    Ok(ExpressionValue::Right(t.const_array(&array_initial_values).to_le_value_enum()))
                }
                LEBasicTypeEnum::Float(t) => {
                    let array_initial_values = array_values.into_iter().map(|v| v.try_into().unwrap()).collect::<Vec<LEFloatValue>>();
                    Ok(ExpressionValue::Right(t.const_array(&array_initial_values).to_le_value_enum()))
                }
                LEBasicTypeEnum::Bool(t) => {
                    let array_initial_values = array_values.into_iter().map(|v| v.try_into().unwrap()).collect::<Vec<LEBoolValue>>();
                    Ok(ExpressionValue::Right(t.const_array(&array_initial_values).to_le_value_enum()))
                }
                LEBasicTypeEnum::Pointer(t) => {
                    let array_initial_values = array_values.into_iter().map(|v| v.try_into().unwrap()).collect::<Vec<LEPointerValue>>();
                    Ok(ExpressionValue::Right(t.const_array(&array_initial_values).to_le_value_enum()))
                }
                LEBasicTypeEnum::Array(t) => {
                    let array_initial_values = array_values.into_iter().map(|v| v.try_into().unwrap()).collect::<Vec<LEArrayValue>>();
                    Ok(ExpressionValue::Right(t.const_array(&array_initial_values).to_le_value_enum()))
                }
                LEBasicTypeEnum::Struct(t) => {
                    let array_initial_values = array_values.into_iter().map(|v| v.try_into().unwrap()).collect::<Vec<LEStructValue>>();
                    Ok(ExpressionValue::Right(t.const_array(&array_initial_values).to_le_value_enum()))
                }
                LEBasicTypeEnum::Vector(t) => {
                    let array_initial_values = array_values.into_iter().map(|v| v.try_into().unwrap()).collect::<Vec<LEVectorValue>>();
                    Ok(ExpressionValue::Right(t.const_array(&array_initial_values).to_le_value_enum()))
                }
            }
        }
    }

    // fn build_string_literal(&mut self, value: &StringLiteral) -> Result<LEBasicValueEnum<'ctx>> {
    //     self.generator.context.llvm_context.const_string()
    // }

    fn build_binary_operator_expression(&mut self, value: &BinaryOpExpression) -> Result<ExpressionValue<'ctx>> {
        match value.op {
            Operator::Plus => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Right(self.generator.build_add(left, right)?))
            }
            Operator::Sub => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Right(self.generator.build_sub(left, right)?))
            }
            Operator::Mul => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Right(self.generator.build_mul(left, right)?))
            }
            Operator::Div => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Right(self.generator.build_div(left, right)?))
            }
            Operator::Assign => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Left(self.generator.build_assign(left, right)?))
            }
            Operator::Equal => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Right(self.generator.build_compare(left, right, CompareBinaryOperator::Equal)?.to_le_value_enum()))
            }
            Operator::NotEqual => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Right(self.generator.build_compare(left, right, CompareBinaryOperator::NotEqual)?.to_le_value_enum()))
            }
            Operator::GreaterThan => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Right(self.generator.build_compare(left, right, CompareBinaryOperator::GreaterThan)?.to_le_value_enum()))
            }
            Operator::LessThan => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Right(self.generator.build_compare(left, right, CompareBinaryOperator::LessThan)?.to_le_value_enum()))
            }
            Operator::GreaterOrEqualThan => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Right(self.generator.build_compare(left, right, CompareBinaryOperator::GreaterOrEqualThan)?.to_le_value_enum()))
            }
            Operator::LessOrEqualThan => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Right(self.generator.build_compare(left, right, CompareBinaryOperator::LessOrEqualThan)?.to_le_value_enum()))
            }
            Operator::Dot => {
                let left = self.build_expression(value.left.as_ref())?;
                if let Expr::Identifier(identifier) = value.right.as_ref() {
                    Ok(ExpressionValue::Left(self.generator.build_dot(left, &identifier.name)?))
                } else {
                    Err(CompileError::NoSuitableBinaryOperator {
                        op: Operator::Dot,
                        left: "".to_string(),
                        right: "".to_string(),
                    })
                }
            }
            Operator::And => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Right(self.generator.build_binary_logic(left, right, LogicBinaryOperator::LogicAnd)?.to_le_value_enum()))
            }
            Operator::Or => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Right(self.generator.build_binary_logic(left, right, LogicBinaryOperator::LogicOr)?.to_le_value_enum()))
            }
            Operator::Xor => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Right(self.generator.build_binary_logic(left, right, LogicBinaryOperator::LogicXor)?.to_le_value_enum()))
            }

            Operator::Mod => {
                let left = self.build_expression(value.left.as_ref())?;
                let right = self.build_expression(value.right.as_ref())?;
                Ok(ExpressionValue::Right(self.generator.build_mod(left, right)?.to_le_value_enum()))
            }
            _ => { unimplemented!() }
        }
    }

    fn build_identifier_expression(&mut self, value: &Identifier) -> Result<ExpressionValue<'ctx>> {
        match value.name.as_str() {
            "true" => { Ok(ExpressionValue::Right(self.generator.context.bool_type().const_true_value().to_le_value_enum())) }
            "false" => { Ok(ExpressionValue::Right(self.generator.context.bool_type().const_false_value().to_le_value_enum())) }
            _ => { Ok(ExpressionValue::Left(self.generator.get_variable(&value.name)?)) }
        }
    }

    fn build_number_literal_expression(&mut self, value: &NumberLiteral) -> Result<ExpressionValue<'ctx>> {
        match value.number {
            Number::Integer(i) => {
                let ty = self.generator.context.i32_type();
                let value = ty.get_llvm_type().const_int(i, true);
                Ok(ExpressionValue::Right(LEIntegerValue { ty, llvm_value: value }.to_le_value_enum()))
            }
            Number::Float(f) => {
                let ty = self.generator.context.double_type();
                let value = ty.get_llvm_type().const_float(f);
                Ok(ExpressionValue::Right(LEFloatValue { ty, llvm_value: value }.to_le_value_enum()))
            }
        }
    }

    fn build_call_expression(&mut self, value: &FunctionCall) -> Result<ExpressionValue<'ctx>> {
        let function = self.generator.context.compiler_context.get_function(&value.function_name)?;
        let mut params = vec![];
        for param in value.params.iter() {
            params.push(self.build_expression(param)?)
        }
        self.generator.build_call(function, &params)
    }

    fn build_local_variable_definition(&mut self, value: &Variable) -> Result<ExpressionValue<'ctx>> {
        let initial_value = self.build_expression(value.value.as_ref())?;
        if let Some(variable_type) = &value.prototype.type_declarator {
            self.generator.create_local_variable_with_exact_type(value.prototype.name.clone(), initial_value, variable_type)?;
        } else {
            self.generator.create_local_variable(value.prototype.name.clone(), initial_value)?;
        }
        Ok(ExpressionValue::Unit)
    }

    fn build_code_block(&mut self, code_block: &CodeBlock) -> Result<bool> {
        for statement in code_block.statements.iter() {
            match statement {
                Statement::Expressions(expr) => {
                    self.build_expression(expr)?;
                }
                Statement::Return(expr) => {
                    let value = self.build_expression(expr)?;
                    self.build_return(value)?;
                    return Ok(true);
                }
                Statement::If(if_expr) => {
                    self.build_if_statement(if_expr)?;
                }
                Statement::ForLoop(for_loop) => {
                    self.build_for_loop(for_loop)?;
                }
                Statement::VariableDefinition(variable_definition) => {
                    self.build_local_variable_definition(variable_definition)?;
                }
                Statement::Void => {}
                Statement::WhileLoop(while_loop) => {
                    self.build_while_loop(while_loop)?;
                }
            }
        }
        Ok(false)
    }

    fn build_return(&mut self, value: ExpressionValue) -> Result<()> {
        let return_variable = &self.generator.context.compiler_context.return_variable;
        let return_block = self.generator.context.compiler_context.return_block.unwrap();
        if let Some(return_variable) = return_variable {
            self.generator.build_store(return_variable.clone(), value);
        }
        self.generator.context.llvm_builder.build_unconditional_branch(return_block);
        Ok(())
    }

    fn build_for_loop(&mut self, for_loop: &ForLoop) -> Result<()> {
        let loop_variable = for_loop.init_statement.as_ref();

        if let Statement::Expressions(cond_expr) = for_loop.condition.as_ref() {
            let cond_block = self.generator.context.llvm_context.insert_basic_block_after(self.generator.context.llvm_builder.get_insert_block().unwrap(), "");
            let body_block = self.generator.context.llvm_context.insert_basic_block_after(cond_block, "");
            let after_block = self.generator.context.llvm_context.insert_basic_block_after(body_block, "");
            self.generator.context.compiler_context.push_block_table();
            if let Statement::VariableDefinition(v) = loop_variable {
                self.build_local_variable_definition(v)?;
            }
            self.generator.context.llvm_builder.build_unconditional_branch(cond_block);
            self.generator.context.llvm_builder.position_at_end(cond_block);
            let cond = self.build_expression(cond_expr.as_ref())?;
            if let LEBasicValueEnum::Bool(bool_cond) = self.generator.read_expression_value(cond)? {
                self.generator.context.llvm_builder.build_conditional_branch(bool_cond.get_llvm_value(), body_block, after_block);
            } else {
                return Err(CompileError::TypeMismatched { expect: "".into(), found: "".into() });
            }
            self.generator.context.llvm_builder.position_at_end(body_block);
            self.build_code_block(&for_loop.code_block)?;

            if let Statement::Expressions(step_expr) = for_loop.iterate.as_ref() {
                self.build_expression(step_expr.as_ref())?;
            }
            self.generator.context.llvm_builder.build_unconditional_branch(cond_block);
            self.generator.context.llvm_builder.position_at_end(after_block);
            self.generator.context.compiler_context.pop_block_table();
        }
        Ok(())
    }

    fn build_while_loop(&mut self, while_loop: &WhileLoop) -> Result<()> {
        let cond_block = self.generator.context.llvm_context.insert_basic_block_after(self.generator.context.llvm_builder.get_insert_block().unwrap(), "");
        let body_block = self.generator.context.llvm_context.insert_basic_block_after(cond_block, "");
        let after_block = self.generator.context.llvm_context.insert_basic_block_after(body_block, "");
        self.generator.context.llvm_builder.build_unconditional_branch(cond_block);
        self.generator.context.llvm_builder.position_at_end(cond_block);
        self.generator.context.compiler_context.push_block_table();
        if let Some(cond_expr) = &while_loop.condition {
            let cond = self.build_expression(cond_expr.as_ref())?;
            if let LEBasicValueEnum::Bool(bool_cond) = self.generator.read_expression_value(cond)? {
                self.generator.context.llvm_builder.build_conditional_branch(bool_cond.get_llvm_value(), body_block, after_block);
            } else {
                return Err(CompileError::TypeMismatched { expect: "".into(), found: "".into() });
            }
        } else {
            self.generator.context.llvm_builder.build_unconditional_branch(body_block);
        }
        self.generator.context.llvm_builder.position_at_end(body_block);
        self.build_code_block(&while_loop.code_block)?;
        self.generator.context.llvm_builder.build_unconditional_branch(cond_block);
        self.generator.context.llvm_builder.position_at_end(after_block);
        self.generator.context.compiler_context.pop_block_table();
        Ok(())
    }

    fn build_if_statement(&mut self, statement: &IfStatement) -> Result<()> {
        let then_block = self.generator.context.llvm_context.insert_basic_block_after(self.generator.context.llvm_builder.get_insert_block().unwrap(), "");
        let else_block = self.generator.context.llvm_context.insert_basic_block_after(then_block, "");
        let merge_block = self.generator.context.llvm_context.insert_basic_block_after(else_block, "");
        let cond_value = self.build_expression(statement.cond.as_ref())?;
        if let LEBasicValueEnum::Bool(bool_cond) = self.generator.read_expression_value(cond_value)? {
            self.generator.context.llvm_builder.build_conditional_branch(bool_cond.get_llvm_value(), then_block, else_block);
        } else {
            return Err(CompileError::TypeMismatched { expect: "".into(), found: "".into() });
        }
        self.generator.context.llvm_builder.position_at_end(then_block);
        self.generator.context.compiler_context.push_block_table();
        let is_then_return_block = self.build_code_block(&statement.then_block)?;
        if !is_then_return_block {
            self.generator.context.llvm_builder.build_unconditional_branch(merge_block);
        }
        self.generator.context.llvm_builder.position_at_end(else_block);
        if let Some(el) = &statement.else_block {
            let is_else_return_block = self.build_code_block(el)?;
            if !is_else_return_block {
                self.generator.context.llvm_builder.build_unconditional_branch(merge_block);
            }
        } else {
            self.generator.context.llvm_builder.build_unconditional_branch(merge_block);
        }
        self.generator.context.llvm_builder.position_at_end(merge_block);
        self.generator.context.compiler_context.pop_block_table();
        Ok(())
    }

    fn build_function_prototype(&mut self, module: &Module<'ctx>, prototype: &FunctionPrototype) -> Result<LEFunctionValue<'ctx>> {
        let mut param_llvm_metadata_types = vec![];
        let mut param_types = vec![];
        for param_type in prototype.param_types.iter() {
            let ty = self.generator.get_generic_type(param_type)?;
            param_types.push(ty.clone());
            param_llvm_metadata_types.push(BasicMetadataTypeEnum::from(ty.get_llvm_basic_type()))
        }
        let mut return_type;
        let external_function = match &prototype.return_type {
            None => {
                return_type = None;
                self.generator.context.llvm_context.void_type().fn_type(&param_llvm_metadata_types, false)
            }
            Some(type_declarator) => {
                let ty = self.generator.get_generic_type(type_declarator)?;
                return_type = Some(ty.clone());
                match ty {
                    LEBasicTypeEnum::Integer(i) => { i.get_llvm_type().fn_type(&param_llvm_metadata_types, false) }
                    LEBasicTypeEnum::Bool(i) => { i.get_llvm_type().fn_type(&param_llvm_metadata_types, false) }
                    LEBasicTypeEnum::Float(i) => { i.get_llvm_type().fn_type(&param_llvm_metadata_types, false) }
                    LEBasicTypeEnum::Pointer(i) => { i.get_llvm_type().fn_type(&param_llvm_metadata_types, false) }
                    LEBasicTypeEnum::Array(i) => { i.get_llvm_type().fn_type(&param_llvm_metadata_types, false) }
                    LEBasicTypeEnum::Struct(i) => { i.get_llvm_type().fn_type(&param_llvm_metadata_types, false) }
                    LEBasicTypeEnum::Vector(i) => { i.get_llvm_type().fn_type(&param_llvm_metadata_types, false) }
                }
            }
        };
        let external_function_value = module.add_function(&prototype.name, external_function, Some(Linkage::External));
        let function_type = LEFunctionType::new(external_function, return_type, param_types);
        let le_function = LEFunctionValue { ty: function_type, llvm_value: external_function_value };
        self.generator.insert_global_function(prototype.name.clone(), le_function.clone())?;
        Ok(le_function)
    }

    fn build_return_block(&mut self, return_block: BasicBlock, return_variable: Option<LEPointerValue>) -> Result<()> {
        self.generator.context.llvm_builder.position_at_end(return_block);
        if let Some(value) = return_variable {
            let value = self.generator.build_load(value)?;
            self.generator.context.llvm_builder.build_return(Some(&value.to_llvm_basic_value_enum()));
            Ok(())
        } else {
            self.generator.context.llvm_builder.build_return(None);
            Ok(())
        }
    }

    fn build_function(&mut self, module: &Module<'ctx>, function_node: &FunctionDefinition) -> Result<LEFunctionValue<'ctx>> {
        let function_value = self.build_function_prototype(module, &function_node.prototype)?;
        let entry = self.generator.context.llvm_context.append_basic_block(function_value.llvm_value, "");
        let return_block = self.generator.context.llvm_context.append_basic_block(function_value.llvm_value, "");
        let return_type = function_value.ty.return_type();
        if let Some(none_void_type) = return_type {
            self.generator.context.llvm_builder.position_at_end(entry);
            let return_variable = self.generator.build_alloca_without_initialize(none_void_type)?;
            self.generator.context.compiler_context.set_current_context(function_value.llvm_value, Some(return_variable.clone()), return_block);
            self.generator.context.llvm_builder.position_at_end(return_block);
            self.build_return_block(return_block, Some(return_variable.clone()))?;
        } else {
            self.generator.context.compiler_context.set_current_context(function_value.llvm_value, None, return_block);
            self.build_return_block(return_block, None)?;
        }
        self.generator.context.llvm_builder.position_at_end(entry);
        self.generator.context.compiler_context.push_block_table();
        let function = &function_value;
        let names = &function_node.param_names;
        for ((param, name), param_type) in function.llvm_value.get_param_iter().zip(names).zip(function.ty.param_types().iter()) {
            let param_value = LEBasicValueEnum::from_type_and_llvm_value(param_type.clone(), param)?;
            self.create_local_variable(name, param_type.clone(), ExpressionValue::Right(param_value))?;
        }

        let is_return_block = self.build_code_block(&function_node.code_block)?;
        if !is_return_block {
            self.generator.context.llvm_builder.build_unconditional_branch(return_block);
        }
        self.generator.context.compiler_context.pop_block_table();
        Ok(function_value)
    }


    pub fn create_local_variable(&mut self, name: &str, target_type: LEBasicTypeEnum<'ctx>, initial_value: ExpressionValue<'ctx>) -> Result<LEPointerValue<'ctx>> {
        let current_insert_block = self.generator.context.llvm_builder.get_insert_block().unwrap();
        let parent_function = self.generator.context.compiler_context.current_function.unwrap();
        let entry_block = parent_function.get_first_basic_block().unwrap();
        if let Some(first_instruction) = entry_block.get_first_instruction() {
            self.generator.context.llvm_builder.position_at(entry_block, &first_instruction);
        } else {
            self.generator.context.llvm_builder.position_at_end(entry_block);
        }
        let le_variable = self.generator.create_local_variable(name.into(), initial_value)?;
        self.generator.context.llvm_builder.position_at_end(current_insert_block);
        Ok(le_variable)
    }

    fn generate_all_functions(&mut self, module: &Module<'ctx>, ast: &Ast) -> Result<()> {
        for function_prototype in ast.extern_functions.iter() {
            let name = function_prototype.name.clone();
            self.build_function_prototype(module, function_prototype)?;
        }
        for function_node in ast.function_definitions.iter() {
            let name = function_node.prototype.name.clone();
            self.build_function(module, function_node)?;
        }
        Ok(())
    }

    fn generate_all_global_variables(&mut self, module: &Module<'ctx>, ast: &Ast) -> Result<()> {
        for variable in ast.globals_variables.iter() {
            let expr_value = self.build_expression(variable.value.as_ref())?;
            if let Some(exact_type) = &variable.prototype.type_declarator {
                let ty = self.generator.get_generic_type(exact_type)?;
                self.generator.create_global_variable_with_exact_type(
                    variable.prototype.name.clone(),
                    expr_value,
                    ty,
                    module,
                )?;
            } else {
                self.generator.create_global_variable(
                    variable.prototype.name.clone(),
                    expr_value,
                    module,
                )?;
            }
        }
        Ok(())
    }

    pub fn compile(&mut self, module: &Module<'ctx>, ast: &Ast) -> Result<()> {
        self.generate_all_global_variables(module, ast)?;
        self.generate_all_global_structures(module, ast)?;
        self.generate_all_functions(module, ast)?;
        Ok(())
    }

    pub fn create(context: &'ctx Context) -> Self {
        let llvm_builder = context.create_builder();
        Self {
            generator: LEGenerator::new(context, llvm_builder),
            current_pos: Position { line: 0 },
        }
    }
    fn generate_all_global_structures(&mut self, module: &Module, ast: &Ast) -> Result<()> {
        for structure in ast.globals_structures.iter() {
            let mut names = vec![];
            let mut types = vec![];
            for (name, ty) in structure.members.iter() {
                names.push(name.as_str());
                types.push(self.generator.get_generic_type(ty)?);
            }
            let structure_type = LEStructType::from_llvm_type(&self.generator.context, &names, &types);
            self.generator.insert_global_type(structure.name.clone(), structure_type.to_le_type_enum())?;
        }
        Ok(())
    }
}


