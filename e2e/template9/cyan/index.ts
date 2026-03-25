import { GlobType, StartTemplateWithLambda } from '@atomicloud/cyan-sdk';
import { IInquirer, IDeterminism } from '@atomicloud/cyan-sdk';
StartTemplateWithLambda(async (i: IInquirer, d: IDeterminism) => {
  // Simple question to ensure coordinator creates area directory for build
  const greeting = await i.text('Greeting', 'template9/greeting');

  return {
    processors: [
      {
        name: 'cyane2e/processor2',
        files: [
          {
            glob: '**/*',
            exclude: [],
            type: GlobType.Template,
            root: 'template',
          },
        ],
        config: {
          vars: { greeting },
        },
      },
    ],
    plugins: [],
  };
});
