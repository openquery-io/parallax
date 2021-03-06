use futures::future::ready;
use sqlparser::ast;

use crate::common::*;

use super::{
    expr::*, CompositionError, Context, ContextKey, ExprAnsatz, ExprRepr, RebaseExpr, RelAnsatz,
    RelRepr, ToAnsatz, ToContext, TryToContext, ValidateError, ValidateResult,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum JoinConstraint<E = ExprT> {
    On(E),
    Using(Vec<ContextKey>),
    Natural,
}

impl<E> JoinConstraint<E> {
    pub async fn map_expressions_async<'a, O, F, Fut>(&'a self, f: F) -> JoinConstraint<O>
    where
        F: Fn(&'a E) -> Fut,
        Fut: Future<Output = O> + 'a,
    {
        match self {
            JoinConstraint::On(e) => JoinConstraint::On(f(e).await),
            JoinConstraint::Using(what) => JoinConstraint::Using(what.clone()),
            JoinConstraint::Natural => JoinConstraint::Natural,
        }
    }
    pub fn map_expressions<'a, O: 'a, F: Fn(&'a E) -> O>(&'a self, f: &F) -> JoinConstraint<O> {
        match self {
            JoinConstraint::On(e) => JoinConstraint::On(f(e)),
            JoinConstraint::Using(what) => JoinConstraint::Using(what.clone()),
            JoinConstraint::Natural => JoinConstraint::Natural,
        }
    }
}

impl<V, E> JoinConstraint<std::result::Result<V, E>> {
    pub fn into_result_expressions(self) -> std::result::Result<JoinConstraint<V>, E> {
        let res = match self {
            JoinConstraint::On(e) => JoinConstraint::On(e?),
            JoinConstraint::Using(what) => JoinConstraint::Using(what),
            JoinConstraint::Natural => JoinConstraint::Natural,
        };
        Ok(res)
    }
}

impl<E> TryInto<ast::JoinConstraint> for JoinConstraint<E>
where
    E: ToAnsatz<Ansatz = ExprAnsatz>,
{
    type Error = CompositionError;
    fn try_into(self) -> Result<ast::JoinConstraint, Self::Error> {
        let out = match self {
            Self::On(expr) => ast::JoinConstraint::On(expr.to_ansatz()?.into()),
            Self::Using(key) => {
                let key = key.into_iter().map(|ck| ck.name().to_string()).collect();
                ast::JoinConstraint::Using(key)
            }
            Self::Natural => ast::JoinConstraint::Natural,
        };
        Ok(out)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum JoinOperator<E = ExprT> {
    Inner(JoinConstraint<E>),
    LeftOuter(JoinConstraint<E>),
    RightOuter(JoinConstraint<E>),
    FullOuter(JoinConstraint<E>),
    CrossJoin,
}

impl<E> TryInto<ast::JoinOperator> for JoinOperator<E>
where
    E: ToAnsatz<Ansatz = ExprAnsatz>,
{
    type Error = CompositionError;
    fn try_into(self) -> Result<ast::JoinOperator, Self::Error> {
        let out = match self {
            JoinOperator::Inner(cst) => ast::JoinOperator::Inner(cst.try_into()?),
            JoinOperator::LeftOuter(cst) => ast::JoinOperator::LeftOuter(cst.try_into()?),
            JoinOperator::RightOuter(cst) => ast::JoinOperator::RightOuter(cst.try_into()?),
            JoinOperator::FullOuter(cst) => ast::JoinOperator::FullOuter(cst.try_into()?),
            JoinOperator::CrossJoin => ast::JoinOperator::CrossJoin,
        };
        Ok(out)
    }
}

impl<V, E> JoinOperator<std::result::Result<V, E>> {
    pub fn into_result_expressions(self) -> std::result::Result<JoinOperator<V>, E> {
        let res = match self {
            JoinOperator::Inner(inner) => JoinOperator::Inner(inner.into_result_expressions()?),
            JoinOperator::LeftOuter(inner) => {
                JoinOperator::LeftOuter(inner.into_result_expressions()?)
            }
            JoinOperator::RightOuter(inner) => {
                JoinOperator::RightOuter(inner.into_result_expressions()?)
            }
            JoinOperator::FullOuter(inner) => {
                JoinOperator::FullOuter(inner.into_result_expressions()?)
            }
            JoinOperator::CrossJoin => JoinOperator::CrossJoin,
        };
        Ok(res)
    }
}

impl<E> JoinOperator<E> {
    pub async fn map_expressions_async<'a, O, F, Fut>(&'a self, f: F) -> JoinOperator<O>
    where
        F: Fn(&'a E) -> Fut,
        Fut: Future<Output = O> + Send + 'a,
    {
        match self {
            JoinOperator::Inner(inner) => JoinOperator::Inner(inner.map_expressions_async(f).await),
            JoinOperator::LeftOuter(inner) => {
                JoinOperator::LeftOuter(inner.map_expressions_async(f).await)
            }
            JoinOperator::RightOuter(inner) => {
                JoinOperator::RightOuter(inner.map_expressions_async(f).await)
            }
            JoinOperator::FullOuter(inner) => {
                JoinOperator::FullOuter(inner.map_expressions_async(f).await)
            }
            JoinOperator::CrossJoin => JoinOperator::CrossJoin,
        }
    }
    pub fn map_expressions<'a, O: 'a, F: Fn(&'a E) -> O>(&'a self, f: &F) -> JoinOperator<O> {
        match self {
            JoinOperator::Inner(inner) => JoinOperator::Inner(inner.map_expressions(f)),
            JoinOperator::LeftOuter(inner) => JoinOperator::LeftOuter(inner.map_expressions(f)),
            JoinOperator::RightOuter(inner) => JoinOperator::RightOuter(inner.map_expressions(f)),
            JoinOperator::FullOuter(inner) => JoinOperator::FullOuter(inner.map_expressions(f)),
            JoinOperator::CrossJoin => JoinOperator::CrossJoin,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Order {
    Asc,
    Desc,
}

copy_ast_enum!(
    #[derive(Serialize, Deserialize, Debug, Clone,)]
    pub enum ast::SetOperator as SetOperator {
        Union,
        Except,
        Intersect,
    }
);

entish! {
    #[derive(Map, MapOwned, From, TryInto, IntoResult, IntoOption)]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[entish(variants_as_structs)]
    pub enum GenericRel<Expr> {
        Aggregation {
            pub attributes: Vec<Expr>,
            pub group_by: Vec<Expr>,
            pub from: Self
        },
        Projection {
            pub attributes: Vec<Expr>,
            pub from: Self,
        },
        Selection {
            pub from: Self,
            pub where_: Expr
        },
        Join {
            pub left: Self,
            pub right: Self,
            pub operator: JoinOperator<Expr>
        },
        Set {
            pub operator: SetOperator,
            pub left: Self,
            pub right: Self
        },
        Offset {
            pub offset: Expr,
            pub from: Self
        },
        Limit {
            pub number_rows: Expr,
            pub from: Self
        },
        OrderBy {
            pub order: Vec<Order>,
            pub by: Vec<Expr>,
            pub from: Self
        },
        Distinct {
            pub from: Self
        },
        WithAlias {
            pub from: Self,
            pub alias: String
        },
        Table(pub ContextKey)
    }
}

to_ansatz! {
    match<Expr,> GenericRel<RelAnsatz> {
        OrderBy<Expr,> { order, by, from } => {
            let mut out: ast::Query = from.into();
            out.order_by = by
                .into_iter()
                .zip(order.into_iter())
                .map(|(by, order)| {
                    let asc = match order {
                        Order::Asc => true,
                        Order::Desc => false
                    };
                    let obe = ast::OrderByExpr {
                        asc: Some(asc),
                        expr: by.to_ansatz()?.into()
                    };
                    Ok(obe)
                })
                .collect::<Result<Vec<_>, _>>()?;
            out
        },
        Distinct<> { from } => {
            let mut out: ast::Select = from.into();
            out.distinct = true;
            out
        },
        Limit<Expr,> { number_rows, from } => {
            let mut out: ast::Query = from.into();
            out.limit = Some(number_rows.to_ansatz()?.into());
            out
        },
        Offset<Expr,> { offset, from } => {
            let mut out: ast::Query = from.into();
            out.offset = Some(offset.to_ansatz()?.into());
            out
        },
        Set<> { operator, left, right } => {
            ast::SetExpr::SetOperation {
                op: operator.into(),
                all: false,
                left: Box::new(left.into()),
                right: Box::new(right.into())
            }
        },
        Join<Expr,> { left, right, operator } => {
            let mut out: ast::TableWithJoins = left.into();
            let join = ast::Join {
                relation: right.into(),
                join_operator: operator.try_into()?
            };
            out.joins.push(join);
            out
        },
        Selection<Expr,> { from, where_ } => {
            let mut out: ast::Select = from.wrapped();
            out.selection = Some(where_.to_ansatz()?.into());
            out
        },
        Aggregation<Expr,> { attributes, group_by, from } => {
            let mut out: ast::Select = from.wrapped();
            out.projection = attributes
                .into_iter()
                .map(|attr| Ok(attr.to_ansatz()?.into()))
                .collect::<Result<Vec<_>, _>>()?;
            out.group_by = group_by
                .into_iter()
                .map(|attr| Ok(attr.to_ansatz()?.into()))
                .collect::<Result<Vec<_>, _>>()?;
            out
        },
        Projection<Expr,> { attributes, from } => {
            let mut out: ast::Select = from.wrapped();
            out.projection = attributes
                .into_iter()
                .map(|attr| Ok(attr.to_ansatz()?.into()))
                .collect::<Result<Vec<_>, _>>()?;
            out
        },
        WithAlias<> { from, alias } => {
            from.with_alias(&alias)
        },
        #[leaf] Table<>(key) => {
            let mut ident = key.0;
            ident.reverse();
            ast::TableFactor::Table {
                name: ast::ObjectName(ident),
                alias: None,
                args: vec![],
                with_hints: vec![]
            }
        },
    }
}

pub type Rel<T> = GenericRel<ExprT, T>;

/// Relation Tree
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RelT<B = TableMeta, E = ExprT> {
    pub(crate) root: GenericRel<E, Arc<RelT<B, E>>>,
    pub(crate) board: ValidateResult<B>,
}

// This stuff is boilerplate and should be in Entish
impl<E, C> GenericRel<E, C>
where
    E: Send + Sync + Clone,
    C: Send + Sync + Clone,
{
    pub async fn map_async<'a, F, Fut, CC>(&'a self, f: F) -> GenericRel<E, CC>
    where
        F: Fn(&'a C) -> Fut,
        Fut: Future<Output = CC> + Send + 'a,
    {
        map_variants!(
            self as GenericRel {
                Aggregation => {
                    attributes: { attributes.clone() },
                    group_by: { group_by.clone() },
                    from: { f(from).await },
                },
                Projection => {
                    attributes: { attributes.clone() },
                    from: { f(from).await },
                },
                Selection => {
                    from: { f(from).await },
                    where_: { where_.clone() },
                },
                Offset => {
                    offset: { offset.clone() },
                    from: { f(from).await },
                },
                Limit => {
                    number_rows: { number_rows.clone() },
                    from: { f(from).await },
                },
                OrderBy => {
                    order: { order.clone() },
                    by: { by.clone() },
                    from: { f(from).await },
                },
                Join => {
                    left: { f(left).await },
                    right: { f(right).await },
                    operator: { operator.clone() },
                },
                Set => {
                    operator: { operator.clone() },
                    left: { f(left).await },
                    right: { f(right).await },
                },
                Distinct => {
                    from: { f(from).await },
                },
                WithAlias => {
                    from: { f(from).await },
                    alias: { alias.clone() },
                },
                #[unnamed] Table => {
                    context_key: { context_key.clone() },
                },
            }
        )
    }
    pub fn map_expressions_async<'a, F, Fut, EE>(
        &'a self,
        f: F,
    ) -> impl Future<Output = GenericRel<EE, C>> + Send + 'a
    where
        EE: Send + Sync,
        F: Fn(&'a E) -> Fut + Send + Sync + 'a,
        Fut: Future<Output = EE> + Send + 'a,
    {
        async move {
            map_variants!(
                self as GenericRel {
                    Aggregation => {
                        attributes: { join_all(attributes.iter().map(|elt| f(elt))).await },
                        group_by: { join_all(group_by.iter().map(|elt| f(elt))).await },
                        from: { from.clone() },
                    },
                    Projection => {
                        attributes: { join_all(attributes.iter().map(|elt| f(elt))).await },
                        from: { from.clone() },
                    },
                    Selection => {
                        from: { from.clone() },
                        where_: { f(where_).await },
                    },
                    Offset => {
                        offset: { f(offset).await },
                        from: { from.clone() },
                    },
                    Limit => {
                        number_rows: { f(number_rows).await },
                        from: { from.clone() },
                    },
                    OrderBy => {
                        order: { order.clone() },
                        by: { join_all(by.iter().map(|elt| f(elt))).await },
                        from: { from.clone() },
                    },
                    Join => {
                        left: { left.clone() },
                        right: { right.clone() },
                        operator: { operator.map_expressions_async(f).await },
                    },
                    Set => {
                        operator: { operator.clone() },
                        left: { left.clone() },
                        right: { right.clone() },
                    },
                    Distinct => {
                        from: { from.clone() },
                    },
                    WithAlias => {
                        from: { from.clone() },
                        alias: { alias.clone() },
                    },
                    #[unnamed] Table => {
                        context_key: { context_key.clone() },
                    },
                }
            )
        }
    }
    pub fn map_expressions<'a, O: 'a, F: Fn(&'a E) -> O>(&'a self, f: &F) -> GenericRel<O, C> {
        map_variants!(
            self as GenericRel {
                Aggregation => {
                    attributes: { attributes.iter().map(f).collect() },
                    group_by: { group_by.iter().map(f).collect() },
                    from: { from.clone() },
                },
                Projection => {
                    attributes: { attributes.iter().map(f).collect() },
                    from: { from.clone() },
                },
                Selection => {
                    from: { from.clone() },
                    where_: { f(where_) },
                },
                Offset => {
                    offset: { f(offset) },
                    from: { from.clone() },
                },
                Limit => {
                    number_rows: { f(number_rows) },
                    from: { from.clone() },
                },
                OrderBy => {
                    order: { order.clone() },
                    by: { by.iter().map(f).collect() },
                    from: { from.clone() },
                },
                Join => {
                    left: { left.clone() },
                    right: { right.clone() },
                    operator: { operator.map_expressions(f) },
                },
                Set => {
                    operator: { operator.clone() },
                    left: { left.clone() },
                    right: { right.clone() },
                },
                Distinct => {
                    from: { from.clone() },
                },
                WithAlias => {
                    from: { from.clone() },
                    alias: { alias.clone() },
                },
                #[unnamed] Table => {
                    context_key: { context_key.clone() },
                },
            }
        )
    }
}

impl<V, E, T> GenericRel<std::result::Result<V, E>, T> {
    pub fn into_result_expressions(self) -> std::result::Result<GenericRel<V, T>, E> {
        let res = map_variants!(
            self as GenericRel {
                Aggregation => {
                    attributes: { attributes.into_iter().collect::<Result<Vec<_>, E>>()? },
                    group_by: { group_by.into_iter().collect::<Result<Vec<_>, E>>()? },
                    from: { from },
                },
                Projection => {
                    attributes: { attributes.into_iter().collect::<Result<Vec<_>, E>>()? },
                    from: { from },
                },
                Selection => {
                    from: { from },
                    where_: { where_? },
                },
                Offset => {
                    offset: { offset? },
                    from: { from },
                },
                Limit => {
                    number_rows: { number_rows? },
                    from: { from },
                },
                OrderBy => {
                    order: { order },
                    by: { by.into_iter().collect::<Result<Vec<_>, E>>()? },
                    from: { from },
                },
                Join => {
                    left: { left },
                    right: { right },
                    operator: { operator.into_result_expressions()? },
                },
                Set => {
                    operator: { operator },
                    left: { left },
                    right: { right },
                },
                Distinct => {
                    from: { from },
                },
                WithAlias => {
                    from: { from },
                    alias: { alias },
                },
                #[unnamed] Table => {
                    context_key: { context_key },
                },
            }
        );
        Ok(res)
    }
}

impl<B, E> RelT<B, E> {
    pub fn is_leaf(&self) -> bool {
        match &self.root {
            GenericRel::Table(..) => true,
            _ => false,
        }
    }
}

/// A representation of the RelT algebra
pub trait Repr: Send + Sync {
    type ExprRepr: ExprRepr;
    type RelRepr: RelRepr<Self::ExprRepr>;
    fn to_inner_context<E>(root: GenericRel<&E, &Self::RelRepr>) -> Context<Self::ExprRepr>;
}

/// A rule for rebasing a relation tree. Note that rebasing a relation tree
/// means also rebasing all the wrapped expression trees.
pub trait RebaseRel<'a, From: Repr>: Send + Sync {
    type To: Repr;
    /// To what do we want to rebase the given node `at`? If returns `None`, we do not
    /// touch it and, instead, the recursion carries on.
    fn rebase_at(
        &'a self,
        at: &'a Relation<From>,
    ) -> Pin<Box<dyn Future<Output = Option<Relation<Self::To>>> + Send + 'a>>;

    fn rebase(
        &'a self,
        root: &'a Relation<From>,
    ) -> Pin<Box<dyn Future<Output = Relation<Self::To>> + Send + 'a>>
    where
        <Self::To as Repr>::RelRepr: Clone + Send + Sync + 'static,
        <Self::To as Repr>::ExprRepr: Clone + Send + Sync + 'static,
    {
        self.rebase_at(root)
            .then(move |res| {
                if let Some(new_base) = res {
                    ready(new_base).boxed()
                } else {
                    root.root
                        .map_async(move |child| self.rebase(child))
                        .map(|rebased_root| {
                            let inherited = rebased_root
                                .map_expressions(&|expr| expr)
                                .map(&mut |child| child.board.as_ref())
                                .into_result()
                                .map(|root| Self::To::to_inner_context(root))
                                .unwrap_or_default();
                            let inherited_ref = &inherited;
                            let rebased_root =
                                rebased_root.map_expressions(&move |expr: &ExprT<
                                    From::ExprRepr,
                                >| {
                                    inherited_ref.rebase(expr)
                                });
                            RelT::from(rebased_root)
                        })
                        .boxed()
                }
            })
            .boxed()
    }
}

pub struct RebaseClosure<'a, I, C, Fut, O> {
    _input: PhantomData<&'a I>,
    closure: C,
    _fut: PhantomData<Fut>,
    _output: PhantomData<O>,
}

impl<'a, I, C, Fut, O> RebaseClosure<'a, I, C, Fut, O>
where
    I: Repr,
    C: Fn(&'a I::RelRepr, &'a ContextKey) -> Fut,
    Fut: Future<Output = ValidateResult<O::RelRepr>> + 'a,
    O: Repr,
{
    pub fn new(closure: C) -> Self {
        Self {
            _input: PhantomData,
            closure,
            _fut: PhantomData,
            _output: PhantomData,
        }
    }
}

// FIXME: Potentially a refactor of RebaseClosure could handle this?
unsafe impl<'a, I, C, Fut, O> Sync for RebaseClosure<'a, I, C, Fut, O> where C: Sync {}

impl<'a, From, C, Fut, To> RebaseRel<'a, From> for RebaseClosure<'a, From, C, Fut, To>
where
    From: Repr,
    To: Repr,
    C: Fn(&'a From::RelRepr, &'a ContextKey) -> Fut + Send + Sync,
    Fut: Future<Output = ValidateResult<To::RelRepr>> + Send + 'a,
{
    type To = To;
    fn rebase_at(
        &'a self,
        at: &'a Relation<From>,
    ) -> Pin<Box<dyn Future<Output = Option<Relation<Self::To>>> + Send + 'a>> {
        async move {
            match &at.root {
                GenericRel::Table(Table(context_key)) => Some(RelT {
                    root: GenericRel::Table(Table(context_key.clone())),
                    board: match at.board.as_ref() {
                        Err(e) => Err(e.clone()),
                        Ok(board) => (self.closure)(board, context_key).await,
                    },
                }),
                _ => None,
            }
        }
        .boxed()
    }
}

impl<'a, From, To> RebaseRel<'a, From> for Context<To>
where
    From: Repr + 'a,
    To: Repr<RelRepr = To> + RelRepr<<To as Repr>::ExprRepr> + Clone,
{
    type To = To;
    fn rebase_at(
        &'a self,
        at: &'a Relation<From>,
    ) -> Pin<Box<dyn Future<Output = Option<Relation<Self::To>>> + Send + 'a>> {
        async move {
            let getter = async move |_, context_key| {
                self.get(context_key)
                    .map(|r| r.clone())
                    .map_err(|e| e.into_table_error())
            };
            let rebase_ = RebaseClosure::<From, _, _, To>::new(getter);
            rebase_.rebase_at(at).await
        }
        .boxed()
    }
}

pub type Relation<R: Repr> = RelT<R::RelRepr, ExprT<R::ExprRepr>>;

impl<R, E> RelT<R, ExprT<E>>
where
    R: RelRepr<E>,
    E: ExprRepr,
{
    pub fn from<T: Into<GenericRel<ExprT<E>, Self>>>(t: T) -> Self {
        let rel = t.into();
        let root = rel.map_owned(&mut |c| Arc::new(c));
        Self::from_wrapped(root)
    }
    pub fn from_wrapped<T: Into<GenericRel<ExprT<E>, Arc<Self>>>>(t: T) -> Self {
        let root = t.into();
        let board = try {
            let inherited: GenericRel<_, &R> = root
                .map(&mut |c| c.board.as_ref())
                .into_result()
                .map_err(|e| e.clone())?;
            let as_ref: GenericRel<_, &R> =
                inherited.map_expressions(&|expr: &ExprT<E>| expr.board.as_ref());
            let flattened = as_ref.into_result_expressions().map_err(|e| e.clone())?;
            R::dot(flattened)?
        };
        Self { root, board }
    }
}

impl TryToContext for RelT {
    type M = ExprMeta;
    fn try_to_context(&self) -> ValidateResult<Context<Self::M>> {
        self.board
            .as_ref()
            .map(|table_meta| table_meta.to_context())
            .map_err(|e| e.clone())
    }
}

impl<B, E> GenericRelTree<E> for RelT<B, E>
where
    B: Clone,
    E: Clone,
{
    fn as_ref(&self) -> GenericRel<E, &Self> {
        self.root.map(&mut |n| n.as_ref())
    }
    fn into_inner(self) -> GenericRel<E, Self> {
        self.root.map_owned(&mut |n| (*n).clone())
    }
}

impl ToAnsatz for RelT {
    type Ansatz = RelAnsatz;
    fn to_ansatz(self) -> Result<Self::Ansatz, CompositionError> {
        self.try_fold(&mut |t| t.to_ansatz())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TableMeta {
    pub columns: Context<ExprMeta>,
    pub loc: Option<BlockType>,
    pub source: Option<ContextKey>,
    pub provenance: Option<ContextKey>,
    pub audience: HashSet<BlockType>,
}

impl TableMeta {
    pub fn from(ctx: Context<ExprMeta>) -> ValidateResult<Self> {
        Ok(Self {
            columns: ctx,
            ..Default::default()
        })
    }
}

impl ToContext for TableMeta {
    type M = ExprMeta;
    fn to_context(&self) -> Context<Self::M> {
        self.columns.clone()
    }
}

impl RelRepr<ExprMeta> for TableMeta {
    fn dot(node: GenericRel<&ExprMeta, &Self>) -> ValidateResult<Self> {
        let inherited = node.map(&mut |child| &child.columns);
        let columns: Context<ExprMeta> = Context::dot(inherited)?;

        let mut locs = HashSet::new();
        node.map(&mut |table_meta| {
            if let Some(loc) = table_meta.loc.as_ref() {
                locs.insert(loc);
            }
        });

        let source = None;

        let loc = if locs.len() == 1 {
            Some((*locs.iter().next().unwrap()).clone())
        } else {
            None
        };

        let mut provenances = Vec::new();
        match &node {
            GenericRel::Table(Table(context_key)) => {
                provenances.push(context_key);
            }
            _ => {
                node.map(&mut |child| {
                    if let Some(provenance) = child.provenance.as_ref() {
                        provenances.push(provenance);
                    }
                });
            }
        };
        let provenance = ContextKey::common(provenances);

        let mut audiences = Vec::new();
        match node {
            GenericRel::Projection(Projection { attributes, .. }) => attributes
                .iter()
                .for_each(|expr_meta| audiences.push(&expr_meta.audience)),
            _ => {
                node.map(&mut |child| {
                    audiences.push(&child.audience);
                });
            }
        };

        let mut audience = audiences
            .pop()
            .map(|aud| aud.clone())
            .unwrap_or(HashSet::new());

        for aud in audiences.into_iter() {
            audience = audience.intersection(aud).cloned().collect();
        }

        Ok(Self {
            columns,
            loc,
            source,
            provenance,
            audience,
        })
    }
}

impl Repr for TableMeta {
    type ExprRepr = ExprMeta;
    type RelRepr = Self;
    fn to_inner_context<E>(root: GenericRel<&E, &Self::RelRepr>) -> Context<Self::ExprRepr> {
        let mut ctx = Context::new();
        root.map(&mut |child| ctx.extend(child.to_context()));
        ctx
    }
}
