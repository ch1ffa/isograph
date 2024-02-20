import type {ReaderArtifact, ReaderAst} from '@isograph/react';
import { PetFavoritePhrase as resolver } from '../../../FavoritePhrase.tsx';

// the type, when read out (either via useLazyReference or via graph)
export type Query__PetFavoritePhrase__outputType = (React.FC<any>);

const readerAst: ReaderAst<Query__PetFavoritePhrase__param> = [
  {
    kind: "Linked",
    fieldName: "pet",
    alias: null,
    arguments: [
      [
        "id",
        { kind: "Variable", name: "id" },
      ],
    ],
    selections: [
      {
        kind: "Scalar",
        fieldName: "name",
        alias: null,
        arguments: null,
      },
      {
        kind: "Scalar",
        fieldName: "favorite_phrase",
        alias: null,
        arguments: null,
      },
    ],
  },
];

export type Query__PetFavoritePhrase__param = { data:
{
  pet: ({
    name: string,
    favorite_phrase: (string | null),
  } | null),
},
[index: string]: any };

const artifact: ReaderArtifact<
  Query__PetFavoritePhrase__param,
  Query__PetFavoritePhrase__param,
  Query__PetFavoritePhrase__outputType
> = {
  kind: "ReaderArtifact",
  resolver: resolver as any,
  readerAst,
  variant: { kind: "Component", componentName: "Query.PetFavoritePhrase" },
};

export default artifact;
