import { GlobType, StartTemplateWithLambda } from '@atomicloud/cyan-sdk';

StartTemplateWithLambda(async () => {
  return {
    processors: [
      {
        name: 'cyane2e/processor1',
        files: [
          {
            glob: '**/*',
            exclude: [],
            type: GlobType.Copy,
            root: 'template',
          },
        ],
        config: {},
      },
    ],
    plugins: [],
  };
});
