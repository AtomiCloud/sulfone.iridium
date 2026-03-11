import { GlobType, StartTemplateWithLambda } from '@atomicloud/cyan-sdk';
import { IInquirer, IDeterminism } from '@atomicloud/cyan-sdk';
StartTemplateWithLambda(async (i: IInquirer, d: IDeterminism) => {
  return {
    processors: [
      {
        name: 'cyane2e/processor1',
        files: [
          {
            glob: '**/*.*',
            exclude: [],
            type: GlobType.Copy,
            root: 'template',
          },
        ],
        config: {},
      },
      {
        name: 'cyane2e/processor1',
        files: [
          {
            glob: '**/*.*',
            exclude: [],
            type: GlobType.Copy,
            root: 'internal',
          },
        ],
        config: {},
      },
    ],
    plugins: [
      {
        name: 'cyane2e/plugin1',
        config: {},
      },
    ],
  };
});
