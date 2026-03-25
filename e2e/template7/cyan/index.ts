import { GlobType, StartTemplateWithLambda, QuestionType } from '@atomicloud/cyan-sdk';
import { IInquirer, IDeterminism } from '@atomicloud/cyan-sdk';

StartTemplateWithLambda(async (i: IInquirer, d: IDeterminism) => {
  const database = await i.select(
    'Which database to use?',
    ['PostgreSQL', 'MySQL', 'SQLite'],
    'cyane2e/template7/database',
  );

  const port = await i.text('What port to use?', 'cyane2e/template7/port');

  const enableSSL = await i.confirm('Enable SSL?', 'cyane2e/template7/enableSSL');

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
            database,
            port,
            enableSSL: enableSSL ? 'true' : 'false',
          },
        },
      },
    ],
    plugins: [],
  };
});
