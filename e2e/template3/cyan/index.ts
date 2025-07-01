import { GlobType, StartTemplateWithLambda } from '@atomicloud/cyan-sdk';
import { IInquirer, IDeterminism } from '@atomicloud/cyan-sdk';

StartTemplateWithLambda(async (i: IInquirer, d: IDeterminism) => {
  const name = await i.text('What is your name?', 'ernest/template1/name');

  const condition = await i.select(
    'What is your condition?',
    ['Headache', 'Cough', 'Fever', 'Other'],
    'ernest/template3/condition',
  );

  const symptoms = await i.select(
    'What are your symptoms?',
    ['Headache', 'Cough', 'Fever', 'Other'],
    'ernest/template3/symptoms',
  );

  const treatment = await i.select(
    'What is your treatment?',
    ['Medication', 'Surgery', 'Other'],
    'ernest/template3/treatment',
  );

  const medication = await i.select(
    'What is your medication?',
    ['Aspirin', 'Ibuprofen', 'Paracetamol', 'Other'],
    'ernest/template3/medication',
  );

  const prognosis = await i.select('What is your prognosis?', ['Good', 'Bad', 'Other'], 'ernest/template3/prognosis');

  return {
    processors: [
      {
        name: 'ernest/processor1',
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
            name,
            condition,
            symptoms,
            treatment,
            medication,
            prognosis,
          },
        },
      },
    ],
    plugins: [
      {
        name: 'ernest/plugin1',
        config: {},
      },
    ],
  };
});
