import { GlobType, StartTemplateWithLambda, QuestionType } from '@atomicloud/cyan-sdk';
import { IInquirer, IDeterminism } from '@atomicloud/cyan-sdk';

StartTemplateWithLambda(async (i: IInquirer, d: IDeterminism) => {
  // Text question
  const projectName = await i.text('What is your project name?', 'cyane2e/template5/projectName');

  // Password question
  const apiKey = await i.password('Enter your API key:', 'cyane2e/template5/apiKey');

  // Date question
  const startDate = await i.dateSelect({
    type: QuestionType.DateSelect,
    id: 'cyane2e/template5/startDate',
    message: 'When does the project start?',
    default: new Date(2026, 2, 13),
  });

  // Select question
  const language = await i.select(
    'What programming language?',
    ['TypeScript', 'Python', 'Rust', 'Go'],
    'cyane2e/template5/language',
  );

  // Confirm question
  const useDocker = await i.confirm('Use Docker?', 'cyane2e/template5/useDocker');

  // Checkbox question
  const features = await i.checkbox(
    'Which features to enable?',
    ['logging', 'metrics', 'tracing'],
    'cyane2e/template5/features',
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
            projectName,
            apiKey,
            startDate,
            language,
            useDocker: useDocker ? 'true' : 'false',
            features: features.join(','),
          },
        },
      },
    ],
    plugins: [],
  };
});
