import { GlobType, StartTemplateWithLambda } from '@atomicloud/cyan-sdk';
import { IInquirer, IDeterminism } from '@atomicloud/cyan-sdk';
StartTemplateWithLambda(async (i: IInquirer, d: IDeterminism) => {
  const templateName = await i.text('What is the name for the generated template?', 'cyane2e/template6/templateName');

  const authorName = await i.text(
    'What is the author name for the generated cyan.yaml?',
    'cyane2e/template6/authorName',
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
            templateName,
            authorName,
          },
        },
      },
    ],
    plugins: [],
  };
});
