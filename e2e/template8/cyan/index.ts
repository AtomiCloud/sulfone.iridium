import { GlobType, StartTemplateWithLambda } from '@atomicloud/cyan-sdk';
import { IInquirer, IDeterminism } from '@atomicloud/cyan-sdk';

StartTemplateWithLambda(async (i: IInquirer, d: IDeterminism) => {
  const appName = await i.text('What is your app name?', 'cyane2e/template8/appName');

  const environment = await i.select(
    'What environment?',
    ['Development', 'Staging', 'Production'],
    'cyane2e/template8/environment',
  );

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
          vars: {
            appName,
            environment,
          },
        },
      },
    ],
    plugins: [],
  };
});
