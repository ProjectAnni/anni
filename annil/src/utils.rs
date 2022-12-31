use axum::response::{IntoResponse, IntoResponseParts};

pub(crate) enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<L, R> IntoResponseParts for Either<L, R>
where
    L: IntoResponseParts,
    R: IntoResponseParts,
{
    type Error = Either<L::Error, R::Error>;

    fn into_response_parts(
        self,
        res: axum::response::ResponseParts,
    ) -> Result<axum::response::ResponseParts, Self::Error> {
        match self {
            Either::Left(l) => l.into_response_parts(res).map_err(|e| Either::Left(e)),
            Either::Right(r) => r.into_response_parts(res).map_err(|e| Either::Right(e)),
        }
    }
}

impl<L, R> IntoResponse for Either<L, R>
where
    L: IntoResponse,
    R: IntoResponse,
{
    fn into_response(self) -> axum::response::Response {
        match self {
            Either::Left(l) => l.into_response(),
            Either::Right(r) => r.into_response(),
        }
    }
}
