// Based on Dfinity's rust bindings generator:
// https://github.com/dfinity/candid/blob/master/rust/candid/src/bindings/rust.rs

use candid::bindings::analysis::chase_actor;
use candid::bindings::analysis::infer_rec;
use candid::bindings::rust::TypePath;
use candid::parser::typing::CheckFileOptions;
use candid::parser::typing::CheckFileResult;
use candid::types::Field;
use candid::types::FuncMode;
use candid::types::Function;
use candid::types::Label;
use candid::types::Type;
use candid::types::TypeInner;
use candid::TypeEnv;
use convert_case::Case;
use convert_case::Casing;
use instrumented_error::Result;
use quote::__private::TokenStream;
use quote::format_ident;
use quote::quote;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use syn::Ident;

fn is_tuple(fs: &[candid::types::Field]) -> bool {
    if fs.is_empty() {
        return false;
    }
    !fs.iter()
        .enumerate()
        .any(|(i, field)| field.id.get_id() != (i as u32))
}

fn q_ident(id: &str) -> (Ident, bool) {
    if id.is_empty()
        || id.starts_with(|c: char| !c.is_ascii_alphabetic() && c != '_')
        || id.chars().any(|c| !c.is_ascii_alphanumeric() && c != '_')
    {
        (format_ident!("_{}_", candid::idl_hash(id)), true)
    } else if ["crate", "self", "super", "Self"].contains(&id) {
        (format_ident!("_{}", id), true)
    } else {
        (format_ident!("{}", id), false)
    }
}

fn q_field_name(id: &str) -> TokenStream {
    let (ident, is_rename) = q_ident(id);
    if is_rename {
        let id_escape_debug = id.escape_debug().to_string();
        quote!(
            #[serde(rename=#id_escape_debug)]
            #ident
        )
    } else {
        quote!(#ident)
    }
}

fn q_label(id: &Label) -> TokenStream {
    match id {
        Label::Named(str) => q_field_name(str),
        Label::Id(n) | Label::Unnamed(n) => {
            let ident = format_ident!("_{}_", n);
            quote!(#ident)
        }
    }
}

fn q_record_field(field: &candid::types::Field, recs: &BTreeSet<&str>) -> TokenStream {
    let field_name = q_label(&field.id);
    let type_ = q_ty(&field.ty, recs);
    quote!(pub #field_name : #type_)
}

fn q_record_fields(
    fs: &[candid::types::Field],
    recs: &BTreeSet<&str>,
    make_pub: bool,
) -> TokenStream {
    if is_tuple(fs) {
        let fields = fs.iter().map(|f| q_ty(&f.ty, recs));
        // We want to make fields on a tuple public
        // However `q_record_fields` can be called
        // from multiple paths.
        // Valid candid:
        // type ServiceControllers = vec record {
        //      principal;
        //      vec ServiceControllerKind;
        // }
        // In rust translates to
        // type ServiceControllers = Vec<(...)>
        // Here we cannot make fields of the tuple pub
        //
        // Valid candid:
        // type TxLogSerializedEntry = record {
        //      u64;
        //      ByteBuf;
        //  }
        // Rust equivalent
        // pub struct TxLogSerializedEntry = (u64, ByteBuf);
        // Here we need to make tuple entrants pub
        // Or we have no way to access the members / instantiate
        // objects.
        if make_pub {
            quote!((#(pub #fields),*))
        } else {
            quote!((#(#fields),*))
        }
    } else {
        let fields = fs.iter().map(|f| q_record_field(f, recs));
        quote!({#(#fields),*})
    }
}

fn q_variant_field(field: &candid::types::Field, recs: &BTreeSet<&str>) -> TokenStream {
    match &field.ty.as_ref() {
        TypeInner::Null => q_label(&field.id),
        TypeInner::Record(fs) => {
            let label = q_label(&field.id);
            let fields = q_record_fields(fs, recs, false);
            quote!(#label #fields)
        }
        _ => {
            let label = q_label(&field.id);
            let field = q_ty(&field.ty, recs);
            quote!(#label(#field))
        }
    }
}

fn q_ty(ty: &Type, recs: &BTreeSet<&str>) -> TokenStream {
    use TypeInner::*;
    match ty.as_ref() {
        Null => quote!(()),
        Bool => quote!(bool),
        Nat => quote!(candid::Nat),
        Int => quote!(candid::Int),
        Nat8 => quote!(u8),
        Nat16 => quote!(u16),
        Nat32 => quote!(u32),
        Nat64 => quote!(u64),
        Int8 => quote!(i8),
        Int16 => quote!(i16),
        Int32 => quote!(i32),
        Int64 => quote!(i64),
        Float32 => quote!(f32),
        Float64 => quote!(f64),
        Text => quote!(String),
        Reserved => quote!(candid::Reserved),
        Empty => quote!(candid::Empty),
        Var(ref id) => {
            let name = q_ident(id).0;
            if recs.contains(id.as_str()) {
                quote!(Box<#name>)
            } else {
                quote!(#name)
            }
        }
        Principal => quote!(candid::Principal),
        Opt(ref t) => {
            let nested = q_ty(t, recs);
            quote!(Option<#nested>)
        }
        Vec(ref t) => {
            let nested = q_ty(t, recs);
            quote!(Vec<#nested>)
        }
        Record(ref fs) => q_record_fields(fs, recs, false),
        Variant(_) => unreachable!(), // not possible after rewriting
        Func(_) => quote!(candid::Func),
        Service(_) => quote!(candid::Service),
        Class(_, _) => unreachable!(),
        Knot(_) | Unknown => unreachable!(),
        Future => unreachable!(),
    }
}

fn q_function(id: &str, func: &Function) -> TokenStream {
    let name = q_ident(id).0;
    let empty = BTreeSet::new();
    let func_args = func.args.iter().enumerate().map(|(i, ty)| {
        let arg_ident = format_ident!("arg{i}");
        let type_ = q_ty(ty, &empty);
        quote!(#arg_ident: #type_)
    });
    let args = [quote!(agent: &dscvr_canister_agent::CanisterAgent)]
        .into_iter()
        .chain(func_args);

    let rets = func.rets.iter().map(|ty| q_ty(ty, &empty));

    let arg_names = func.args.iter().enumerate().map(|(i, _ty)| {
        let arg_ident = format_ident!("arg{i}");
        quote!(#arg_ident)
    });

    let agent_call: TokenStream = if func.modes.iter().any(|m| m == &FuncMode::Query) {
        quote!(agent.query(#id, args).await?.as_slice())
    } else {
        quote!(agent.update(#id, args).await?.as_slice())
    };

    let rets_decode = [agent_call].into_iter().chain(rets.clone());

    quote!(
        #[tracing::instrument(skip_all)]
        pub async fn #name(#(#args),*) -> instrumented_error::Result<(#(#rets),*)> {
            let args = candid::Encode!(#(&#arg_names),*)?;
            Ok(candid::Decode!(#(#rets_decode),*)?)
        }
    )
}

#[tracing::instrument(skip_all)]
fn generate_types(env: &TypeEnv, def_list: &[&str], recs: &BTreeSet<&str>) -> Result<TokenStream> {
    let mut ret = TokenStream::default();
    let derive = quote!(
        #[derive(Debug, Clone, PartialEq, Eq, candid::CandidType, serde::Deserialize, serde::Serialize, deepsize::DeepSizeOf)]
    );
    def_list
        .iter()
        .map(|id| {
            let ty = env.find_type(id).expect("type");
            let name = q_ident(id).0;
            match ty.as_ref() {
                TypeInner::Record(fs) => {
                    let fields = q_record_fields(fs, recs, true);
                    let separator = if is_tuple(fs) { quote!(;) } else { quote!() };
                    quote!(
                        #derive
                        pub struct #name #fields
                        #separator
                    )
                }
                TypeInner::Variant(fs) => {
                    if fs
                        .iter()
                        .any(|f| f.id.to_string() == "Ok" || f.id.to_string() == "Err")
                    {
                        let rets = fs.iter().map(|f| q_ty(&f.ty, &BTreeSet::default()));
                        quote!(
                            pub type #name = std::result::Result<#(#rets),*>;
                        )
                    } else {
                        let fields = fs.iter().map(|f| q_variant_field(f, recs));
                        quote!(
                            #derive
                            pub enum #name {
                                #(#fields,)*
                            }
                        )
                    }
                }
                _ => {
                    let field = q_ty(ty, recs);
                    if recs.contains(id) {
                        // unit tuple struct
                        quote!(
                            #derive
                            pub struct #name(pub #field);
                        )
                    } else {
                        // type alias
                        quote!(type #name = #field;)
                    }
                }
            }
        })
        .for_each(|tokens| ret.extend(tokens));
    Ok(ret)
}

fn path_to_var(path: &[TypePath]) -> String {
    let name: Vec<String> = path
        .iter()
        .map(|node| match node {
            TypePath::Id(id) => id.to_string(),
            TypePath::RecordField(f) | TypePath::VariantField(f) => {
                f.to_string().to_case(Case::Title)
            }
            TypePath::Opt => "Inner".to_owned(),
            TypePath::Vec => "Item".to_owned(),
            TypePath::Func(id) => id.to_string(),
            TypePath::Init => "Init".to_owned(),
        })
        .collect();
    name.join("")
}

// Convert structural typing to nominal typing to fit Rust's type system
fn nominalize(env: &mut TypeEnv, path: &mut Vec<TypePath>, t: Type) -> Type {
    match t.as_ref() {
        TypeInner::Opt(ty) => {
            path.push(TypePath::Opt);
            let ty = nominalize(env, path, ty.to_owned());
            path.pop();
            TypeInner::Opt(ty).into()
        }
        TypeInner::Vec(ty) => {
            path.push(TypePath::Opt);
            let ty = nominalize(env, path, ty.to_owned());
            path.pop();
            TypeInner::Vec(ty).into()
        }
        TypeInner::Record(fs) => {
            if matches!(
                path.last(),
                None | Some(TypePath::VariantField(_)) | Some(TypePath::Id(_))
            ) || is_tuple(fs)
            {
                let fs: Vec<_> = fs
                    .iter()
                    .map(|Field { id, ty }| {
                        path.push(TypePath::RecordField(id.to_string()));
                        let ty = nominalize(env, path, ty.to_owned());
                        path.pop();
                        Field {
                            id: id.to_owned(),
                            ty,
                        }
                    })
                    .collect();
                TypeInner::Record(fs).into()
            } else {
                let new_var = path_to_var(path);
                let ty = nominalize(
                    env,
                    &mut vec![TypePath::Id(new_var.clone())],
                    TypeInner::Record(fs.to_owned()).into(),
                );
                env.0.insert(new_var.clone(), ty);
                TypeInner::Var(new_var).into()
            }
        }
        TypeInner::Variant(fs) => match path.last() {
            None | Some(TypePath::Id(_)) => {
                let fs: Vec<_> = fs
                    .iter()
                    .map(|Field { id, ty }| {
                        path.push(TypePath::VariantField(id.to_string()));
                        let ty = nominalize(env, path, ty.to_owned());
                        path.pop();
                        Field {
                            id: id.to_owned(),
                            ty,
                        }
                    })
                    .collect();
                TypeInner::Variant(fs).into()
            }
            Some(_) => {
                let new_var = path_to_var(path);
                let ty = nominalize(
                    env,
                    &mut vec![TypePath::Id(new_var.clone())],
                    TypeInner::Variant(fs.to_owned()).into(),
                );
                env.0.insert(new_var.clone(), ty);
                TypeInner::Var(new_var).into()
            }
        },
        TypeInner::Func(func) => TypeInner::Func(Function {
            modes: func.modes.clone(),
            args: func
                .args
                .iter()
                .enumerate()
                .map(|(i, ty)| {
                    path.push(TypePath::Func(format!("arg{}", i)));
                    let ty = nominalize(env, path, ty.to_owned());
                    path.pop();
                    ty
                })
                .collect(),
            rets: func
                .rets
                .iter()
                .enumerate()
                .map(|(i, ty)| {
                    path.push(TypePath::Func(format!("ret{}", i)));
                    let ty = nominalize(env, path, ty.to_owned());
                    path.pop();
                    ty
                })
                .collect(),
        })
        .into(),
        TypeInner::Service(serv) => TypeInner::Service(
            serv.iter()
                .map(|(meth, ty)| {
                    path.push(TypePath::Id(meth.to_string()));
                    let ty = nominalize(env, path, ty.to_owned());
                    path.pop();
                    (meth.to_owned(), ty)
                })
                .collect(),
        )
        .into(),
        TypeInner::Class(args, ty) => TypeInner::Class(
            args.iter()
                .map(|ty| {
                    path.push(TypePath::Init);
                    let ty = nominalize(env, path, ty.to_owned());
                    path.pop();
                    ty
                })
                .collect(),
            nominalize(env, path, ty.to_owned()),
        )
        .into(),
        _ => t,
    }
}

fn nominalize_all(env: &TypeEnv, actor: &Option<Type>) -> (TypeEnv, Option<Type>) {
    let mut res = TypeEnv(Default::default());
    for (id, ty) in env.0.iter() {
        let ty = nominalize(&mut res, &mut vec![TypePath::Id(id.clone())], ty.clone());
        res.0.insert(id.to_string(), ty);
    }
    let actor = actor
        .as_ref()
        .map(|ty| nominalize(&mut res, &mut vec![], ty.clone()));
    (res, actor)
}

#[tracing::instrument(skip(tokens))]
fn generate_file(path: &Path, tokens: TokenStream) -> Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(b"// @generated\n")?;
    file.write_all(b"#![allow(unused)]\n")?;
    file.write_all(b"#![allow(non_camel_case_types)]\n")?;
    file.write_all(b"#![allow(clippy::upper_case_acronyms)]\n")?;
    // TODO: the vec_box should not be needed
    file.write_all(b"#![allow(clippy::vec_box)]\n")?;
    file.write_all(b"#![allow(clippy::large_enum_variant)]\n")?;
    file.write_all(b"use candid::{Encode, Decode};\n")?;

    let tokens_string = tokens.to_string();
    let syn_file = syn::parse_file(&tokens_string)?;
    file.write_all(prettyplease::unparse(&syn_file).as_bytes())?;

    Ok(())
}

#[tracing::instrument]
pub fn generate(did: &Path, output: &Path) -> Result<BTreeSet<PathBuf>> {
    let CheckFileResult {
        types,
        actor,
        imports,
    } = candid::parser::typing::check_file_with_options(
        did,
        &CheckFileOptions {
            pretty_errors: false,
            combine_actors: true,
        },
    )?;
    let (env, actor) = nominalize_all(&types, &actor);
    let def_list: Vec<_> = if let Some(actor) = &actor {
        chase_actor(&env, actor).unwrap()
    } else {
        env.0.iter().map(|pair| pair.0.as_ref()).collect()
    };
    let recs = infer_rec(&env, &def_list)?;
    let mut tokens = generate_types(&env, &def_list, &recs)?;

    if let Some(actor) = actor {
        let serv = env.as_service(&actor).unwrap();
        serv.iter()
            .map(|(id, func)| {
                let func = env.as_func(func).unwrap();
                q_function(id, func)
            })
            .for_each(|f| tokens.extend(f));
    }

    generate_file(output, tokens)?;
    Ok(imports)
}
